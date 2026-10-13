# OpenDataLoader Agents SDK tool

This package exports `parse_pdf`, an OpenAI Agents SDK function tool for local
PDF conversion. It supports `json` and `markdown` and returns the generated
file contents. It requires no model call or API key.

```python
from agents import Agent
from opendataloader_agents import parse_pdf

agent = Agent(name="PDF reader", instructions="Read local PDFs.", tools=[parse_pdf])
```
