# Meet EvoReview

**EvoReview** is a new open-source project for **self-evolving AI code review**.

Most review bots call an LLM once and ship whatever comes back — noise included. EvoReview treats the *review harness* as the thing that should learn: prompt overlays, precision bias, and a lesson library improve from contrast metrics (anchor rate, spam rate, severity balance) after every run.

> Reviews that get sharper with every run.

## What you get

- **Webhook service** for GitLab / GitHub / Gitea (Push + MR/PR)
- **Inline + summary comments** back to the platform
- **IM notifications** (DingTalk / WeCom / Feishu)
- **Dashboard** for review history and stats
- **Local CLI** for pre-push self-review
- **Harness²** — Propose → Probe → Compose self-evolution (optional dual probe)

## Who it’s for

Teams that want fewer false positives and sharper findings over time — without fine-tuning model weights.

## Start here

- Product README: [README.en.md](./README.en.md) · [README.zh-CN.md](./README.zh-CN.md)
- Brand: [BRANDING.md](./BRANDING.md)
- Self-evolution deep dive: [rust/HARNESS2.md](./rust/HARNESS2.md)
- GitHub: https://github.com/linuslin-cto/evoreview
