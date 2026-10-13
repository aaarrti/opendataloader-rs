set shell := ["zsh", "-cu"]

cfr_version := "0.152"
cfr_jar := "target/tools/cfr-" + cfr_version + ".jar"
cfr_sha256 := "f686e8f3ded377d7bc87d216a90e9e9512df4156e75b06c655a16648ae8765b2"
cfr_release_url := "https://github.com/leibnitz27/cfr/releases/download/" + cfr_version + "/cfr-" + cfr_version + ".jar"

download-cfr:
    mkdir -p target/tools && curl --fail --location --show-error --retry 3 --output "{{cfr_jar}}.part" "{{cfr_release_url}}" && printf '%s  %s\n' "{{cfr_sha256}}" "{{cfr_jar}}.part" | sha256sum --check && mv "{{cfr_jar}}.part" "{{cfr_jar}}"

decompile-cfr jar output_dir:
    java -jar "{{cfr_jar}}" "{{jar}}" --outputdir "{{output_dir}}"

actor-critic-loop:
    uv run actor-critic-loop --task .codex/TASK.md --max-iterations 100 \
    --actor-model gpt-5.6-luna --critic-model gpt-5.6-sol \
    --actor-reasoning-level medium --critic-reasoning-level high

ralph-loop:
    uv run ralph-loop --task .codex/TASK.md --progress .codex/PROGRESS.md \
    --model gpt-5.6-luna --reasoning-effort medium \
    --max-iterations 30



py_src := "packages/opendataloader/src/opendataloader"
clib_name := "libopendataloder_clib.so"
so_name := "libodl.so"



setup:
    uv sync --dev
    uv run dvc pull

lint:
    cargo clippy --workspace --all-targets --locked -- -D warnings
    uv run ruff check .
    uv run black --check .

test:
    cargo test
    ODL_LIB_PATH="target/debug/{{clib_name}}" pytest

build:
    rm -f "{{py_src}}/{{so_name}}"
    cargo build --release
    cp "target/release/{{clib_name}}" "{{py_src}}/"
    mv "{{py_src}}/{{clib_name}}" "{{py_src}}/{{so_name}}"
    uv build --all-packages

