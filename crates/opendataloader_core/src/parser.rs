use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::Path,
};

use lopdf::{Dictionary, Document as PdfDocument, Object, content::Operation};

use super::model::*;

pub(crate) fn parse_document(path: &Path) -> Result<Document, ConversionError> {
    let bytes = fs::read(path).map_err(|source| ConversionError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if !bytes.starts_with(b"%PDF-") {
        return Err(ConversionError::InvalidInput {
            path: path.to_path_buf(),
            reason: "missing %PDF- header".into(),
        });
    }

    let pdf = PdfDocument::load_mem(&bytes).map_err(|error| {
        let reason = error.to_string();
        if reason.to_ascii_lowercase().contains("password") || reason.to_ascii_lowercase().contains("encrypt") {
            ConversionError::PasswordProtected {
                path: path.to_path_buf(),
            }
        } else {
            ConversionError::InvalidInput {
                path: path.to_path_buf(),
                reason,
            }
        }
    })?;
    if pdf.is_encrypted() {
        return Err(ConversionError::PasswordProtected {
            path: path.to_path_buf(),
        });
    }

    let pages = pdf.get_pages();
    let metadata = read_metadata(&pdf);
    let structure_roles = structure_roles(&pdf);
    let mut parsed_pages = Vec::with_capacity(pages.len());
    for (number, page_id) in pages {
        let (width, height) = page_bounds(&pdf, page_id).map_err(|reason| ConversionError::Processing {
            path: path.to_path_buf(),
            reason,
        })?;
        let parent_key = pdf
            .get_dictionary(page_id)
            .ok()
            .and_then(|page| page.get_deref(b"StructParents", &pdf).ok())
            .and_then(|object| object.as_i64().ok())
            .unwrap_or(-1);
        let chunks = page_chunks(
            &pdf,
            page_id,
            number as usize - 1,
            width,
            height,
            parent_key,
            &structure_roles,
        )
        .map_err(|reason| ConversionError::Processing {
            path: path.to_path_buf(),
            reason,
        })?;
        let page = Page {
            index: number as usize - 1,
            width,
            height,
            chunks,
        };
        parsed_pages.push(page);
    }

    let document = Document {
        file_name: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .into(),
        page_count: parsed_pages.len(),
        metadata,
        pages: parsed_pages,
        elements: Vec::new(),
    };
    Ok(document)
}

fn structure_roles(pdf: &PdfDocument) -> BTreeMap<(i64, u64), String> {
    let Ok(root) = pdf
        .catalog()
        .and_then(|catalog| catalog.get_deref(b"StructTreeRoot", pdf))
        .and_then(Object::as_dict)
    else {
        return BTreeMap::new();
    };
    let Ok(parent_tree) = root.get_deref(b"ParentTree", pdf).and_then(Object::as_dict) else {
        return BTreeMap::new();
    };
    let mut roles = BTreeMap::new();
    collect_structure_roles(pdf, &Object::Dictionary(parent_tree.clone()), &mut roles);
    roles
}

fn collect_structure_roles(pdf: &PdfDocument, object: &Object, roles: &mut BTreeMap<(i64, u64), String>) {
    let Ok(dictionary) = pdf.dereference(object).and_then(|(_, object)| object.as_dict()) else {
        return;
    };
    if let Ok(nums) = dictionary.get_deref(b"Nums", pdf).and_then(Object::as_array) {
        for pair in nums.chunks_exact(2) {
            let Ok(parent_key) = pair[0].as_i64() else { continue };
            let Ok(entries) = pdf.dereference(&pair[1]).and_then(|(_, object)| object.as_array()) else {
                continue;
            };
            for (mcid, entry) in entries.iter().enumerate() {
                let Ok(structure) = pdf.dereference(entry).and_then(|(_, object)| object.as_dict()) else {
                    continue;
                };
                if let Some(role) = structure_role(pdf, &Object::Dictionary(structure.clone())) {
                    roles.insert((parent_key, mcid as u64), role);
                }
            }
        }
    }
    if let Ok(kids) = dictionary.get_deref(b"Kids", pdf).and_then(Object::as_array) {
        for kid in kids {
            collect_structure_roles(pdf, kid, roles);
        }
    }
}

fn structure_role(pdf: &PdfDocument, object: &Object) -> Option<String> {
    let mut current = object.clone();
    for _ in 0..32 {
        let (_, structure) = pdf.dereference(&current).ok()?;
        let dictionary = structure.as_dict().ok()?;
        let role = dictionary
            .get_deref(b"S", pdf)
            .ok()?
            .as_name()
            .ok()
            .map(|name| String::from_utf8_lossy(name).into_owned())?;
        if role != "Span" {
            return Some(role);
        }
        current = dictionary.get(b"P").ok()?.clone();
    }
    None
}

fn read_metadata(pdf: &PdfDocument) -> DocumentMetadata {
    let info = pdf
        .trailer
        .get(b"Info")
        .ok()
        .and_then(|object| object.as_reference().ok())
        .and_then(|id| pdf.get_dictionary(id).ok());
    let value = |key| info.and_then(|dict| dict.get(key).ok()).and_then(pdf_text);
    let mut metadata = DocumentMetadata {
        author: value(b"Author"),
        title: value(b"Title"),
        creation_date: value(b"CreationDate"),
        modification_date: value(b"ModDate"),
    };
    if let Some(xmp) = xmp_metadata(pdf) {
        metadata.author = metadata
            .author
            .or_else(|| xmp_value(&xmp, "pdf:Author").or_else(|| xmp_value(&xmp, "dc:creator")));
        metadata.title = metadata.title.or_else(|| xmp_value(&xmp, "dc:title"));
        metadata.creation_date = metadata.creation_date.or_else(|| xmp_value(&xmp, "xmp:CreateDate"));
        metadata.modification_date = metadata.modification_date.or_else(|| xmp_value(&xmp, "xmp:ModifyDate"));
    }
    metadata
}

fn xmp_metadata(pdf: &PdfDocument) -> Option<String> {
    let catalog = pdf.catalog().ok()?;
    let metadata_id = catalog.get(b"Metadata").ok()?.as_reference().ok()?;
    let stream = pdf.get_object(metadata_id).ok()?.as_stream().ok()?;
    let bytes = stream.decompressed_content().ok()?;
    String::from_utf8(bytes).ok()
}

fn xmp_value(xml: &str, tag: &str) -> Option<String> {
    let start = xml.find(&format!("<{tag}"))?;
    let content_start = xml[start..].find('>')? + start + 1;
    let end = xml[content_start..].find(&format!("</{tag}>"))? + content_start;
    let value = xml[content_start..end].replace(['<', '>'], "");
    (!value.trim().is_empty()).then(|| value.trim().to_owned())
}

fn pdf_text(object: &Object) -> Option<String> {
    object
        .as_str()
        .ok()
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
}

fn normalize_font_name(name: &[u8]) -> String {
    let name = String::from_utf8_lossy(name);
    match name.split_once('+') {
        Some((_, name)) => name.to_owned(),
        None => name.into_owned(),
    }
}

fn decoded_pdf_text(
    pdf: &PdfDocument,
    fonts: &std::collections::BTreeMap<Vec<u8>, &Dictionary>,
    font_name: Option<&Vec<u8>>,
    object: &Object,
) -> Option<String> {
    let bytes = object.as_str().ok()?;
    font_name
        .and_then(|name| fonts.get(name))
        .and_then(|font| font_encoding(pdf, font))
        .and_then(|encoding| encoding.bytes_to_string(bytes).ok())
        .or_else(|| Some(String::from_utf8_lossy(bytes).into_owned()))
}

fn font_encoding<'a>(pdf: &'a PdfDocument, font: &'a Dictionary) -> Option<lopdf::Encoding<'a>> {
    if let Ok(descendants) = font.get_deref(b"DescendantFonts", pdf).and_then(Object::as_array) {
        if let Some(descendant) = descendants.first().and_then(|object| pdf.dereference(object).ok()) {
            if let Ok(descendant) = descendant.1.as_dict() {
                if descendant.has(b"ToUnicode") {
                    return descendant.get_font_encoding(pdf).ok();
                }
            }
        }
    }
    font.get_font_encoding(pdf).ok()
}

fn page_bounds(pdf: &PdfDocument, page_id: lopdf::ObjectId) -> Result<(f64, f64), String> {
    let mut current_id = page_id;
    let mut visited = HashSet::new();
    let box_object = loop {
        if !visited.insert(current_id) {
            return Err("cyclic page-tree Parent chain".into());
        }
        let page = pdf.get_dictionary(current_id).map_err(|error| error.to_string())?;
        if page.has(b"MediaBox") {
            break page.get_deref(b"MediaBox", pdf).map_err(|error| error.to_string())?;
        }
        current_id = page
            .get(b"Parent")
            .and_then(Object::as_reference)
            .map_err(|error| error.to_string())?;
    };
    let values = box_object.as_array().map_err(|error| error.to_string())?;
    if values.len() != 4 {
        return Err("MediaBox must contain four numbers".into());
    }
    let left = values[0].as_float().map_err(|error| error.to_string())? as f64;
    let bottom = values[1].as_float().map_err(|error| error.to_string())? as f64;
    let right = values[2].as_float().map_err(|error| error.to_string())? as f64;
    let top = values[3].as_float().map_err(|error| error.to_string())? as f64;
    Ok((right - left, top - bottom))
}

fn page_chunks(
    pdf: &PdfDocument,
    page_id: lopdf::ObjectId,
    page_index: usize,
    width: f64,
    height: f64,
    parent_key: i64,
    structure_roles: &BTreeMap<(i64, u64), String>,
) -> Result<Vec<ParserChunk>, String> {
    let content = pdf
        .get_and_decode_page_content(page_id)
        .map_err(|error| error.to_string())?;
    let fonts = pdf.get_page_fonts(page_id).map_err(|error| error.to_string())?;
    let mut text_state = TextState::default();
    let mut graphics_stack = Vec::new();
    let mut marked_content_stack = Vec::new();
    let mut path_point = None;
    let mut chunks = Vec::new();
    for (parser_order, operation) in content.operations.iter().enumerate() {
        match operation.operator.as_str() {
            "BT" => text_state.reset_text_position(),
            "q" => graphics_stack.push(text_state.ctm),
            "Q" => {
                if let Some(ctm) = graphics_stack.pop() {
                    text_state.ctm = ctm;
                }
            }
            "cm" => concatenate_matrix(&mut text_state.ctm, &operation.operands),
            "BMC" | "BDC" => {
                marked_content_stack.push((text_state.pdfua_tag.clone(), text_state.structure_id));
                text_state.pdfua_tag = operation
                    .operands
                    .first()
                    .and_then(|object| object.as_name().ok())
                    .map(|name| String::from_utf8_lossy(name).into_owned());
                text_state.structure_id = operation.operands.get(1).and_then(|object| {
                    let dictionary = match object {
                        Object::Reference(id) => pdf.get_object(*id).ok()?.as_dict().ok()?,
                        object => object.as_dict().ok()?,
                    };
                    dictionary.get(b"MCID").ok()?.as_i64().ok().map(|id| id as u64)
                });
                if let Some(mcid) = text_state.structure_id {
                    text_state.pdfua_tag = structure_roles
                        .get(&(parent_key, mcid))
                        .cloned()
                        .or(text_state.pdfua_tag.clone());
                }
            }
            "EMC" => {
                if let Some((tag, id)) = marked_content_stack.pop() {
                    text_state.pdfua_tag = tag;
                    text_state.structure_id = id;
                }
            }
            "Tf" => {
                text_state.font_name = operation
                    .operands
                    .first()
                    .and_then(|object| object.as_name().ok())
                    .map(Vec::from);
                text_state.display_font_name = text_state.font_name.as_ref().and_then(|name| {
                    fonts
                        .get(name)
                        .and_then(|font| font.get_deref(b"BaseFont", pdf).ok())
                        .and_then(|object| object.as_name().ok())
                        .map(|name| normalize_font_name(name))
                });
                text_state.font_size = operation.operands.get(1).and_then(object_number).unwrap_or(12.0);
            }
            "Tm" => set_text_matrix(&mut text_state, &operation.operands),
            "Td" => move_text(&mut text_state, &operation.operands, false),
            "TD" => move_text(&mut text_state, &operation.operands, true),
            "T*" => next_text_line(&mut text_state),
            "Tj" => show_text(
                pdf,
                &fonts,
                &mut text_state,
                operation.operands.last(),
                page_index,
                parser_order,
                &mut chunks,
            ),
            "'" => {
                next_text_line(&mut text_state);
                show_text(
                    pdf,
                    &fonts,
                    &mut text_state,
                    operation.operands.last(),
                    page_index,
                    parser_order,
                    &mut chunks,
                );
            }
            "\"" => {
                next_text_line(&mut text_state);
                if let Some(text) = operation.operands.last() {
                    show_text(
                        pdf,
                        &fonts,
                        &mut text_state,
                        Some(text),
                        page_index,
                        parser_order,
                        &mut chunks,
                    );
                }
            }
            "BI" => chunks.push(ParserChunk::Image(ImageChunk {
                page_index,
                bounds: image_bounds(&text_state.ctm),
                object_reference: None,
                inline_image: operation
                    .operands
                    .first()
                    .and_then(|object| object.as_stream().ok())
                    .and_then(inline_image),
                parser_order,
                structure_id: text_state.structure_id,
                pdfua_tag: text_state.pdfua_tag.clone(),
            })),
            "TJ" => {
                let Some(items) = operation.operands.first().and_then(|object| object.as_array().ok()) else {
                    continue;
                };
                let start = (text_state.x, text_state.y);
                let mut text = String::new();
                for item in items {
                    if let Some(value) = decoded_pdf_text(pdf, &fonts, text_state.font_name.as_ref(), item) {
                        text.push_str(&value);
                        text_state.x += text_width(&value, text_state.effective_font_size());
                    } else if let Some(adjustment) = object_number(item) {
                        text_state.x -= adjustment * text_state.font_size / 1000.0;
                    }
                }
                if !text.is_empty() {
                    push_text_chunk(
                        &mut chunks,
                        page_index,
                        parser_order,
                        text_state.bounds(start, text_state.x),
                        text,
                        &text_state,
                    );
                }
            }
            "Do" => {
                let object_reference = operation
                    .operands
                    .first()
                    .and_then(|object| object.as_name().ok())
                    .map(|name| String::from_utf8_lossy(name).into_owned());
                for bounds in image_bounds_for_operation(pdf, page_id, operation, text_state.ctm) {
                    chunks.push(ParserChunk::Image(ImageChunk {
                        page_index,
                        bounds,
                        object_reference: object_reference.clone(),
                        inline_image: None,
                        parser_order,
                        structure_id: text_state.structure_id,
                        pdfua_tag: text_state.pdfua_tag.clone(),
                    }));
                }
            }
            "m" => path_point = point(&operation.operands).map(|point| transform(&text_state.ctm, point)),
            "l" => {
                if let Some(end) = point(&operation.operands).map(|point| transform(&text_state.ctm, point)) {
                    if let Some(start) = path_point
                        && let Some(bounds) = segment_bounds(start, end)
                    {
                        chunks.push(ParserChunk::LineArt(LineArtChunk {
                            page_index,
                            bounds,
                            parser_order,
                        }));
                    }
                    path_point = Some(end);
                }
            }
            "re" => {
                if let Some(bounds) = rectangle_bounds(&text_state.ctm, &operation.operands) {
                    chunks.push(ParserChunk::LineArt(LineArtChunk {
                        page_index,
                        bounds,
                        parser_order,
                    }));
                }
            }
            _ => {}
        }
    }
    if !chunks.iter().any(|chunk| matches!(chunk, ParserChunk::Text(_))) {
        let text = pdf
            .extract_text(&[(page_index + 1) as u32])
            .map_err(|error| error.to_string())?;
        if !text.trim().is_empty() {
            chunks.push(ParserChunk::Text(TextChunk {
                page_index,
                bounds: page_box(width, height),
                text,
                glyph_order: Vec::new(),
                character_spacing: None,
                font: FontInfo::default(),
                parser_order: content.operations.len(),
                structure_id: text_state.structure_id,
                pdfua_tag: text_state.pdfua_tag.clone(),
            }));
        }
    }
    Ok(chunks)
}

fn inline_image(stream: &lopdf::Stream) -> Option<InlineImage> {
    let width = stream
        .dict
        .get(b"W")
        .or_else(|_| stream.dict.get(b"Width"))
        .ok()?
        .as_i64()
        .ok()? as u32;
    let height = stream
        .dict
        .get(b"H")
        .or_else(|_| stream.dict.get(b"Height"))
        .ok()?
        .as_i64()
        .ok()? as u32;
    let bits = stream
        .dict
        .get(b"BPC")
        .or_else(|_| stream.dict.get(b"BitsPerComponent"))
        .ok()?
        .as_i64()
        .ok()?;
    if bits != 8 {
        return None;
    }
    let colorspace = stream
        .dict
        .get(b"CS")
        .or_else(|_| stream.dict.get(b"ColorSpace"))
        .ok()?
        .as_name()
        .ok()?;
    let channels = match colorspace {
        b"G" | b"Gray" | b"DeviceGray" => 1u8,
        b"RGB" | b"DeviceRGB" => 3u8,
        _ => return None,
    };
    (stream.content.len() == width as usize * height as usize * channels as usize).then(|| InlineImage {
        width,
        height,
        channels,
        data: stream.content.clone(),
    })
}

struct TextState {
    x: f64,
    y: f64,
    line_x: f64,
    line_y: f64,
    leading: f64,
    font_name: Option<Vec<u8>>,
    display_font_name: Option<String>,
    font_size: f64,
    text_scale: f64,
    ctm: [f64; 6],
    structure_id: Option<u64>,
    pdfua_tag: Option<String>,
}

impl TextState {
    fn reset_text_position(&mut self) {
        self.x = 0.0;
        self.y = 0.0;
        self.line_x = 0.0;
        self.line_y = 0.0;
        self.leading = 0.0;
    }

    fn bounds(&self, start: (f64, f64), end_x: f64) -> BoundingBox {
        let size = self.effective_font_size();
        let points = [
            self.transform(start.0, start.1),
            self.transform(end_x, start.1),
            self.transform(start.0, start.1 + size),
            self.transform(end_x, start.1 + size),
        ];
        BoundingBox {
            left: points.iter().map(|point| point.0).fold(f64::INFINITY, f64::min),
            bottom: points.iter().map(|point| point.1).fold(f64::INFINITY, f64::min),
            right: points.iter().map(|point| point.0).fold(f64::NEG_INFINITY, f64::max),
            top: points.iter().map(|point| point.1).fold(f64::NEG_INFINITY, f64::max),
        }
    }

    fn effective_font_size(&self) -> f64 {
        let size = if self.font_size > 0.0 { self.font_size } else { 12.0 };
        size * self.text_scale
    }

    fn transform(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.ctm[0] * x + self.ctm[2] * y + self.ctm[4],
            self.ctm[1] * x + self.ctm[3] * y + self.ctm[5],
        )
    }
}

impl Default for TextState {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            line_x: 0.0,
            line_y: 0.0,
            leading: 0.0,
            font_name: None,
            display_font_name: None,
            font_size: 0.0,
            text_scale: 1.0,
            ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            structure_id: None,
            pdfua_tag: None,
        }
    }
}

fn concatenate_matrix(ctm: &mut [f64; 6], operands: &[Object]) {
    if operands.len() < 6 {
        return;
    }
    let Some(values) = operands.iter().take(6).map(object_number).collect::<Option<Vec<_>>>() else {
        return;
    };
    let [a, b, c, d, e, f] = [values[0], values[1], values[2], values[3], values[4], values[5]];
    let [ca, cb, cc, cd, ce, cf] = *ctm;
    *ctm = [
        ca * a + cc * b,
        cb * a + cd * b,
        ca * c + cc * d,
        cb * c + cd * d,
        ca * e + cc * f + ce,
        cb * e + cd * f + cf,
    ];
}

fn object_number(object: &Object) -> Option<f64> {
    object
        .as_float()
        .ok()
        .map(f64::from)
        .or_else(|| object.as_i64().ok().map(|value| value as f64))
}

fn set_text_matrix(state: &mut TextState, operands: &[Object]) {
    if operands.len() >= 6 {
        let a = object_number(&operands[0]).unwrap_or(1.0);
        let b = object_number(&operands[1]).unwrap_or(0.0);
        state.text_scale = a.hypot(b).max(1.0);
        state.line_x = object_number(&operands[4]).unwrap_or(0.0);
        state.line_y = object_number(&operands[5]).unwrap_or(0.0);
        state.x = state.line_x;
        state.y = state.line_y;
    }
}

fn move_text(state: &mut TextState, operands: &[Object], set_leading: bool) {
    if operands.len() < 2 {
        return;
    }
    let x = object_number(&operands[0]).unwrap_or(0.0);
    let y = object_number(&operands[1]).unwrap_or(0.0);
    if set_leading {
        state.leading = -y;
    }
    state.line_x += x;
    state.line_y += y;
    state.x = state.line_x;
    state.y = state.line_y;
}

fn next_text_line(state: &mut TextState) {
    state.line_y -= state.leading;
    state.x = state.line_x;
    state.y = state.line_y;
}

fn show_text(
    pdf: &PdfDocument,
    fonts: &std::collections::BTreeMap<Vec<u8>, &Dictionary>,
    state: &mut TextState,
    object: Option<&Object>,
    page_index: usize,
    parser_order: usize,
    chunks: &mut Vec<ParserChunk>,
) {
    let Some(object) = object else { return };
    let Some(text) = decoded_pdf_text(pdf, fonts, state.font_name.as_ref(), object) else {
        return;
    };
    if text.is_empty() {
        return;
    }
    let start = (state.x, state.y);
    state.x += text_width(&text, state.effective_font_size());
    push_text_chunk(
        chunks,
        page_index,
        parser_order,
        state.bounds(start, state.x),
        text,
        state,
    );
}

fn push_text_chunk(
    chunks: &mut Vec<ParserChunk>,
    page_index: usize,
    parser_order: usize,
    bounds: BoundingBox,
    text: String,
    state: &TextState,
) {
    chunks.push(ParserChunk::Text(TextChunk {
        page_index,
        bounds,
        text,
        glyph_order: Vec::new(),
        character_spacing: None,
        font: FontInfo {
            name: state.display_font_name.clone().or_else(|| {
                state
                    .font_name
                    .as_ref()
                    .map(|name| String::from_utf8_lossy(name).into_owned())
            }),
            size: (state.font_size > 0.0).then_some(state.effective_font_size()),
            ..Default::default()
        },
        parser_order,
        structure_id: state.structure_id,
        pdfua_tag: state.pdfua_tag.clone(),
    }));
}

fn text_width(text: &str, font_size: f64) -> f64 {
    let size = if font_size > 0.0 { font_size } else { 12.0 };
    text.chars()
        .map(|character| {
            if character.is_whitespace() {
                0.25
            } else if character.is_ascii() {
                0.5
            } else {
                1.0
            }
        })
        .sum::<f64>()
        * size
}

pub(crate) fn page_box(width: f64, height: f64) -> BoundingBox {
    BoundingBox {
        left: 0.0,
        bottom: 0.0,
        right: width,
        top: height,
    }
}

fn image_bounds_for_operation(
    pdf: &PdfDocument,
    page_id: lopdf::ObjectId,
    operation: &Operation,
    ctm: [f64; 6],
) -> Vec<BoundingBox> {
    let Some(name) = operation.operands.first().and_then(|object| object.as_name().ok()) else {
        return Vec::new();
    };
    let Ok((resources, _)) = pdf.get_page_resources(page_id) else {
        return Vec::new();
    };
    let Some(resources) = resources else { return Vec::new() };
    image_bounds_in_resources(pdf, resources, name, ctm)
}

fn image_bounds_in_resources(
    pdf: &PdfDocument,
    resources: &Dictionary,
    name: &[u8],
    ctm: [f64; 6],
) -> Vec<BoundingBox> {
    let Ok(xobjects) = resources.get_deref(b"XObject", pdf).and_then(Object::as_dict) else {
        return Vec::new();
    };
    let Ok(reference) = xobjects.get(name).and_then(Object::as_reference) else {
        return Vec::new();
    };
    let Ok(stream) = pdf.get_object(reference).and_then(Object::as_stream) else {
        return Vec::new();
    };
    let Ok(subtype) = stream.dict.get(b"Subtype").and_then(Object::as_name) else {
        return Vec::new();
    };
    if subtype == b"Image" {
        return vec![image_bounds(&ctm)];
    }
    if subtype != b"Form" {
        return Vec::new();
    }
    let Ok(content_bytes) = stream.decompressed_content() else {
        return Vec::new();
    };
    let Ok(content) = lopdf::content::Content::decode(&content_bytes) else {
        return Vec::new();
    };
    let mut form_ctm = ctm;
    if let Ok(matrix) = stream.dict.get(b"Matrix").and_then(Object::as_array) {
        concatenate_matrix(&mut form_ctm, matrix);
    }
    let form_resources = stream
        .dict
        .get_deref(b"Resources", pdf)
        .ok()
        .and_then(|object| object.as_dict().ok())
        .unwrap_or(resources);
    let mut stack = Vec::new();
    let mut images = Vec::new();
    for operation in content.operations {
        match operation.operator.as_str() {
            "q" => stack.push(form_ctm),
            "Q" => {
                if let Some(saved) = stack.pop() {
                    form_ctm = saved;
                }
            }
            "cm" => concatenate_matrix(&mut form_ctm, &operation.operands),
            "Do" => {
                if let Some(name) = operation.operands.first().and_then(|object| object.as_name().ok()) {
                    images.extend(image_bounds_in_resources(pdf, form_resources, name, form_ctm));
                }
            }
            _ => {}
        }
    }
    images
}

fn point(operands: &[Object]) -> Option<(f64, f64)> {
    Some((object_number(operands.first()?)?, object_number(operands.get(1)?)?))
}

fn transform(matrix: &[f64; 6], (x, y): (f64, f64)) -> (f64, f64) {
    (
        matrix[0] * x + matrix[2] * y + matrix[4],
        matrix[1] * x + matrix[3] * y + matrix[5],
    )
}

fn segment_bounds(start: (f64, f64), end: (f64, f64)) -> Option<BoundingBox> {
    (start != end).then_some(BoundingBox {
        left: start.0.min(end.0),
        bottom: start.1.min(end.1),
        right: start.0.max(end.0),
        top: start.1.max(end.1),
    })
}

fn rectangle_bounds(matrix: &[f64; 6], operands: &[Object]) -> Option<BoundingBox> {
    let left = object_number(operands.first()?);
    let bottom = object_number(operands.get(1)?);
    let width = object_number(operands.get(2)?);
    let height = object_number(operands.get(3)?);
    let points = [
        transform(matrix, (left?, bottom?)),
        transform(matrix, (left? + width?, bottom?)),
        transform(matrix, (left?, bottom? + height?)),
        transform(matrix, (left? + width?, bottom? + height?)),
    ];
    Some(BoundingBox {
        left: points.iter().map(|point| point.0).fold(f64::INFINITY, f64::min),
        bottom: points.iter().map(|point| point.1).fold(f64::INFINITY, f64::min),
        right: points.iter().map(|point| point.0).fold(f64::NEG_INFINITY, f64::max),
        top: points.iter().map(|point| point.1).fold(f64::NEG_INFINITY, f64::max),
    })
}

fn image_bounds(matrix: &[f64; 6]) -> BoundingBox {
    rectangle_bounds(
        matrix,
        &[
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(1),
            Object::Integer(1),
        ],
    )
    .unwrap_or(BoundingBox {
        left: 0.0,
        bottom: 0.0,
        right: 1.0,
        top: 1.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parser_reads_lorem_metadata_pages_and_text() -> anyhow::Result<()> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/pdf/lorem.pdf");
        let document = crate::parse_pdf(&path).map_err(|error| anyhow::anyhow!(error))?;
        assert_eq!(document.page_count, 1);
        assert_eq!(document.metadata.author.as_deref(), Some("leebd-public"));
        assert!(document.metadata.title.is_none());
        assert!(document.pages[0].width > 594.0);
        assert!(
            document.pages[0]
                .chunks
                .iter()
                .any(|chunk| matches!(chunk, ParserChunk::Text(text) if !text.text.is_empty()))
        );
        Ok(())
    }

    #[test]
    fn parser_preserves_positioned_text_bounds() -> anyhow::Result<()> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/pdf/lorem.pdf");
        let document = crate::parse_pdf(&path).map_err(|error| anyhow::anyhow!(error))?;
        let text = document.pages[0]
            .chunks
            .iter()
            .find_map(|chunk| match chunk {
                ParserChunk::Text(text) => Some(text),
                _ => None,
            })
            .ok_or_else(|| anyhow::anyhow!("expected positioned text"))?;
        assert!(text.bounds.left > 0.0);
        assert!(text.bounds.right < document.pages[0].width);
        assert!(text.bounds.top < document.pages[0].height);
        assert!(text.font.size.is_some());
        assert!(
            document
                .elements
                .iter()
                .all(|element| !matches!(element, SemanticElement::Table { .. }))
        );
        Ok(())
    }

    #[test]
    fn parser_distinguishes_invalid_and_password_inputs() {
        let invalid_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/pdf/fake-jpg.pdf");
        assert!(matches!(
            crate::parse_pdf(&invalid_path),
            Err(ConversionError::InvalidInput { .. })
        ));
        let password_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/pdf/password-protected.pdf");
        assert!(matches!(
            crate::parse_pdf(&password_path),
            Err(ConversionError::PasswordProtected { .. })
        ));
    }

    #[test]
    fn parser_preserves_raster_image_chunk_without_text() -> anyhow::Result<()> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/pdf/chinese_scan.pdf");
        let document = crate::parse_pdf(&path).map_err(|error| anyhow::anyhow!(error))?;
        assert!(
            document
                .pages
                .iter()
                .flat_map(|page| page.chunks.iter())
                .any(|chunk| matches!(chunk, ParserChunk::Image(_)))
        );
        assert!(
            !document
                .pages
                .iter()
                .flat_map(|page| page.chunks.iter())
                .any(|chunk| matches!(chunk, ParserChunk::Text(_)))
        );
        Ok(())
    }

    #[test]
    fn parser_resolves_inherited_media_box() -> anyhow::Result<()> {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/stg/1XHKM6rWJGznIJctL_7QzImRi1qOHDkGs.pdf");
        let pdf = PdfDocument::load(&path)?;
        let page_id = pdf
            .get_pages()
            .values()
            .next()
            .copied()
            .ok_or_else(|| anyhow::anyhow!("expected a page"))?;
        assert!(pdf.get_dictionary(page_id)?.get(b"MediaBox").is_err());
        let (width, height) = page_bounds(&pdf, page_id).map_err(|error| anyhow::anyhow!(error))?;
        assert!(width > 0.0);
        assert!(height > 0.0);
        let document = crate::parse_pdf(&path).map_err(|error| anyhow::anyhow!(error))?;
        assert_eq!(document.pages[0].width, width);
        assert_eq!(document.pages[0].height, height);
        Ok(())
    }

    #[test]
    fn parser_decodes_embedded_font_text() -> anyhow::Result<()> {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/stg/10-S1GgfUJW5-zg-Zt-F655FYCp_FEbQj.pdf");
        let document = crate::parse_pdf(&path).map_err(|error| anyhow::anyhow!(error))?;
        let text = document
            .pages
            .iter()
            .flat_map(|page| page.chunks.iter())
            .filter_map(|chunk| match chunk {
                ParserChunk::Text(text) => Some(text.text.as_str()),
                _ => None,
            })
            .collect::<String>();
        assert!(text.contains('感'));
        assert!(!text.chars().any(char::is_control));
        Ok(())
    }

    #[test]
    fn parser_resolves_structure_tree_roles_for_images() -> anyhow::Result<()> {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/stg/10-S1GgfUJW5-zg-Zt-F655FYCp_FEbQj.pdf");
        let pdf = PdfDocument::load(&path)?;
        let roles = structure_roles(&pdf);
        assert!(roles.values().any(|role| role == "P"));
        assert!(roles.values().any(|role| role == "Figure"));
        Ok(())
    }
}
