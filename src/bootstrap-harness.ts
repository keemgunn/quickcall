#!/usr/bin/env node
import { homedir } from "node:os";
import { refreshKnown } from "./harness.js";

const mode = process.argv[2];
if (mode !== "refresh-known") {
  process.stderr.write("usage: qc-bootstrap-harness <refresh-known>\n");
  process.exitCode = 1;
} else {
  await refreshKnown({ home: process.env.HOME ?? homedir() });
}
