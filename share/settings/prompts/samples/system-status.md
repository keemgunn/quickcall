---
description: Summarize current host and session facts from captured shell output.
qc_no_skills: true
---

# Confirm System Status

Report whether this session looks normal for local CLI work. Use only the captured facts below. Do not invent missing values. qc already ran the commands; do not try to run them again.

## Live facts

- Host: !`uname -srm`
- Time: !`date`
- User: !`whoami`
- Directory: !`pwd`
- Uptime: !`uptime`

## Report

- OS family and kernel
- Local time
- User and working directory
- Uptime and load, if present

If a field is empty, say that command produced no output. Do not change files or request further inspection.
