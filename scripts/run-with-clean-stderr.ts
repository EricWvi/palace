import { spawn } from "node:child_process";
const child = spawn("npm", ["run", "test", "-w", "apps/palace-web"], {
  stdio: ["inherit", "inherit", "pipe"],
});
let dirty = false;
child.stderr.on("data", (data) => {
  dirty = true;
  process.stderr.write(data);
});
child.on("error", (error) => {
  console.error(error);
  process.exitCode = 1;
});
child.on("close", (code) => {
  process.exitCode = code || (dirty ? 1 : 0);
});
