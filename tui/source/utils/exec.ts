import { spawn } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { existsSync } from "node:fs";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

function findProjectRoot(start: string): string {
  let current = start;
  while (current !== "/") {
    if (existsSync(join(current, "appimage-install"))) {
      return current;
    }
    const parent = dirname(current);
    if (parent === current) break;
    current = parent;
  }
  return dirname(start);
}

export const SCRIPT_PATH =
  process.env.APPIMAGE_INSTALL_SCRIPT ||
  join(findProjectRoot(__dirname), "appimage-install");

export interface ExecResult {
  stdout: string;
  stderr: string;
  exitCode: number;
}

export function execScript(
  args: string[],
  onStdout?: (line: string) => void,
  onStderr?: (line: string) => void,
): Promise<ExecResult> {
  return new Promise((resolve, reject) => {
    const child = spawn(SCRIPT_PATH, args, {
      env: process.env,
      shell: false,
    });

    let stdout = "";
    let stderr = "";

    child.stdout.on("data", (data: Buffer) => {
      const chunk = data.toString("utf8");
      stdout += chunk;
      onStdout?.(chunk);
    });

    child.stderr.on("data", (data: Buffer) => {
      const chunk = data.toString("utf8");
      stderr += chunk;
      onStderr?.(chunk);
    });

    child.on("error", reject);
    child.on("close", (exitCode) => {
      resolve({ stdout, stderr, exitCode: exitCode ?? 0 });
    });
  });
}

export interface InstalledApp {
  name: string;
  displayName: string;
  version: string;
  size: string;
  updatable: boolean;
}

function stripAnsi(input: string): string {
  // eslint-disable-next-line no-control-regex
  return input.replace(/\x1b\[[0-9;]*m/g, "");
}

export async function listInstalledApps(): Promise<InstalledApp[]> {
  const { stdout } = await execScript(["--list"]);
  const lines = stdout.split("\n");
  const apps: InstalledApp[] = [];

  for (const line of lines) {
    const trimmed = stripAnsi(line.trim());
    if (!trimmed || trimmed.startsWith("::") || trimmed.startsWith("No apps")) {
      continue;
    }

    const match = trimmed.match(
      /^\s*(\S+)\s+(.+?)\s+v([^\s]+)\s+\(([^)]+)\)(\s*\[updatable\])?/,
    );
    if (match) {
      apps.push({
        name: match[1],
        displayName: match[2].trim(),
        version: match[3],
        size: match[4],
        updatable: Boolean(match[5]),
      });
    }
  }

  return apps;
}

export async function checkForUpdates(): Promise<InstalledApp[]> {
  const apps = await listInstalledApps();
  return apps.filter((a) => a.updatable);
}
