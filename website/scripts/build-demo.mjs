import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const cwd = fileURLToPath(new URL("../demo-ui/", import.meta.url));
function run(command, args) {
  const env = { ...process.env };
  if ("NO_COLOR" in env) env.NO_COLOR = "true";
  const result = spawnSync(command, args, { cwd, stdio: "inherit", env });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
run(process.execPath, [`${root}node_modules/@tailwindcss/cli/dist/index.mjs`, "-i", "app.css", "-o", "public/app.css", "--minify"]);
run("trunk", ["build", "--release", "--locked"]);
