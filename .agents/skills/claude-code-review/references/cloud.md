# Codex Cloud setup

Commit this folder at `.agents/skills/claude-code-review` in each repository.
Local personal skills do not sync to cloud tasks.
See [cloud environments](https://learn.chatgpt.com/docs/environments/cloud-environments).

Install the tested Claude Code version during environment setup:

```sh
curl -fsSL https://claude.ai/install.sh | bash -s -- 2.1.295
```

Put `~/.local/bin` on the task PATH. Keep the environment's HTTPS proxy and
CA trust configuration. Allow the required
[Anthropic hosts](https://code.claude.com/docs/en/network-config), including
`api.anthropic.com`, `claude.ai`, `platform.claude.com`, and `downloads.claude.ai`.

In current Codex Cloud, supply one credential through Network secrets:

- Subscription: generate a token yourself with `claude setup-token`; use key
  `CLAUDE_CODE_OAUTH_TOKEN`, scoped to `api.anthropic.com`.
- API billing: use `ANTHROPIC_API_KEY`, scoped to `api.anthropic.com`.
  This overrides subscription billing in print mode.

Never paste credentials into chat, source files, or logs.
Legacy cloud secrets are setup-only; do not persist them to bypass that boundary.

Republish the environment and verify a review in a new task. Until then, cloud
execution remains unverified. Missing CLI, credentials, model access, or network
access means an incomplete review.
