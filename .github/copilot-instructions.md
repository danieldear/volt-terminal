# agent007 guidance for GitHub Copilot

Also honor `AGENTS.md` as the Codex entrypoint and `AGENT007.md` as the primary shared agent007 operating contract for this repository.

- Route non-trivial tasks through agent007 first.
- Prefer skills and workflows before broad free-form planning.
- Use `agent007_etr_list` / `agent007_etr_call` for deterministic extraction/query/status tasks before custom shell/Python parsing.
- Use shell for execution of builds/tests/scripts, then synthesize results through agent007.
- If shell/Python is used as a reader/parser, explain why ETR was not sufficient.
