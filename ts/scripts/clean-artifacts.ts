import * as fs from "node:fs";
import * as path from "node:path";

/**
 * Cross-platform artifact and test output cleaner.
 * Deletes transient test outputs, screenshots, logs, and optionally build artifacts.
 */
function cleanDirectory(targetPath: string, rootDir: string): void {
  const fullPath = path.resolve(rootDir, targetPath);
  if (fs.existsSync(fullPath)) {
    try {
      fs.rmSync(fullPath, { recursive: true, force: true });
      console.log(`[Clean] Deleted: ${targetPath}`);
    } catch (err) {
      console.warn(`[Clean] Could not delete ${targetPath}:`, err);
    }
  }
}

function cleanMatchingFiles(rootDir: string, extensions: string[]): number {
  let count = 0;
  const entries = fs.readdirSync(rootDir, { withFileTypes: true });

  for (const entry of entries) {
    if (
      entry.name === "node_modules" ||
      entry.name === ".git" ||
      entry.name === "target"
    ) {
      continue;
    }
    const fullPath = path.join(rootDir, entry.name);
    if (entry.isDirectory()) {
      count += cleanMatchingFiles(fullPath, extensions);
    } else if (entry.isFile()) {
      const ext = path.extname(entry.name).toLowerCase();
      if (extensions.includes(ext)) {
        try {
          fs.unlinkSync(fullPath);
          console.log(
            `[Clean] Deleted file: ${path.relative(process.cwd(), fullPath)}`,
          );
          count++;
        } catch (err) {
          console.warn(`[Clean] Could not delete ${fullPath}:`, err);
        }
      }
    }
  }
  return count;
}

function run(): void {
  const isAll = process.argv.includes("--all");
  const rootDir = process.cwd();

  console.log(
    `[Clean] Starting artifact cleanup (mode: ${isAll ? "all" : "transient"})...`,
  );

  // Transient test directories & reports
  const transientDirs = [
    "test-results",
    "playwright-report",
    "screenshots",
    "coverage",
    ".nyc_output",
  ];

  for (const dir of transientDirs) {
    cleanDirectory(dir, rootDir);
  }

  // Transient screenshot and log files
  const fileExtensionsToClean = [".log", ".out", ".tmp"];
  const cleanedFiles = cleanMatchingFiles(rootDir, fileExtensionsToClean);

  if (isAll) {
    cleanDirectory("pkg", rootDir);
    cleanDirectory("pkg-web", rootDir);
  }

  console.log(
    `[Clean] Done. Transient directories removed. ${cleanedFiles} transient files cleaned.`,
  );
}

run();
