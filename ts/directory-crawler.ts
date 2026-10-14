import * as fs from "fs";
import * as path from "path";
import {
  DocumentInput,
  SemanticSearchCoordinator,
} from "./semantic-search-coordinator";

export interface CrawlerFilterOptions {
  readonly includeExtensions?: readonly string[];
  readonly excludeDirectories?: readonly string[];
  readonly maxFileSizeBytes?: number;
}

export interface CrawlDiscoveryEvent {
  readonly filePath: string;
  readonly relativePath: string;
  readonly extension: string;
  readonly byteSize: number;
}

export class DirectoryCrawler {
  private readonly coordinator: SemanticSearchCoordinator;
  private readonly includeExtensions: Set<string>;
  private readonly excludeDirectories: Set<string>;
  private readonly maxFileSizeBytes: number;

  constructor(
    coordinator: SemanticSearchCoordinator,
    options?: CrawlerFilterOptions,
  ) {
    this.coordinator = coordinator;
    this.includeExtensions = new Set(
      (
        options?.includeExtensions ?? [
          "txt",
          "md",
          "markdown",
          "json",
          "rs",
          "ts",
          "js",
          "csv",
          "yaml",
          "yml",
          "pdf",
        ]
      ).map((e) => e.toLowerCase().replace(/^\./, "")),
    );
    this.excludeDirectories = new Set(
      (
        options?.excludeDirectories ?? [
          ".git",
          "target",
          "node_modules",
          "pkg",
          "dist",
          "build",
          ".system_generated",
        ]
      ).map((d) => d.toLowerCase()),
    );
    this.maxFileSizeBytes = options?.maxFileSizeBytes ?? 10 * 1024 * 1024; // 10MB
  }

  /**
   * Recursively discover eligible document files inside a root directory
   */
  discoverFiles(rootDir: string): CrawlDiscoveryEvent[] {
    const discovered: CrawlDiscoveryEvent[] = [];
    if (!fs.existsSync(rootDir)) return discovered;

    const traverse = (dir: string) => {
      const entries = fs.readdirSync(dir, { withFileTypes: true });
      for (const entry of entries) {
        const fullPath = path.join(dir, entry.name);
        const nameLower = entry.name.toLowerCase();

        if (entry.isDirectory()) {
          if (!this.excludeDirectories.has(nameLower)) {
            traverse(fullPath);
          }
        } else if (entry.isFile()) {
          const ext = path.extname(entry.name).toLowerCase().replace(/^\./, "");
          if (this.includeExtensions.has(ext)) {
            const stat = fs.statSync(fullPath);
            if (stat.size <= this.maxFileSizeBytes && stat.size > 0) {
              discovered.push({
                filePath: fullPath,
                relativePath: path.relative(rootDir, fullPath),
                extension: ext,
                byteSize: stat.size,
              });
            }
          }
        }
      }
    };

    traverse(rootDir);
    return discovered;
  }

  /**
   * Basic extraction of text runs from PDF stream to avoid utf-8 binary corruption
   */
  static extractPdfText(buffer: Buffer): string {
    const raw = buffer.toString("binary");
    const textRuns: string[] = [];
    const tjRegex = /\(([^)]+)\)\s*(?:Tj|'|")/g;
    let match: RegExpExecArray | null;
    while ((match = tjRegex.exec(raw)) !== null) {
      if (match[1] && match[1].trim()) {
        textRuns.push(match[1].trim());
      }
    }
    const arrayRegex = /\[([^\]]+)\]\s*TJ/g;
    while ((match = arrayRegex.exec(raw)) !== null) {
      const innerTj = /\(([^)]+)\)/g;
      let innerMatch: RegExpExecArray | null;
      while ((innerMatch = innerTj.exec(match[1])) !== null) {
        if (innerMatch[1] && innerMatch[1].trim()) {
          textRuns.push(innerMatch[1].trim());
        }
      }
    }
    if (textRuns.length > 0) {
      return textRuns.join(" ");
    }
    const words = raw.match(/[a-zA-Z0-9_À-ỹ]{3,}/g);
    return words ? words.join(" ") : "";
  }

  /**
   * Crawl a directory and index all discovered documents into the SemanticSearchCoordinator
   */
  async crawlAndIndex(
    rootDir: string,
    options?: {
      chunkSizeChars?: number;
      onProgress?: (indexed: number, total: number) => void;
    },
  ): Promise<{
    indexedCount: number;
    errors: { file: string; error: string }[];
  }> {
    const files = this.discoverFiles(rootDir);
    const chunkSize = options?.chunkSizeChars ?? 300;
    let indexedCount = 0;
    const errors: { file: string; error: string }[] = [];

    for (let i = 0; i < files.length; i++) {
      const file = files[i];
      try {
        let content: string;
        if (file.extension === "pdf") {
          const buf = fs.readFileSync(file.filePath);
          content = DirectoryCrawler.extractPdfText(buf);
        } else {
          content = fs.readFileSync(file.filePath, "utf-8");
        }

        const docId = `file:${file.relativePath.replace(/\\/g, "/")}`;
        const title = path.basename(file.filePath);

        const doc: DocumentInput = {
          id: docId,
          title,
          content,
          file_path: file.filePath,
          metadata: {
            extension: file.extension,
            relative_path: file.relativePath,
          },
        };

        await this.coordinator.ingestDocument(doc, chunkSize);
        indexedCount++;
        options?.onProgress?.(indexedCount, files.length);
      } catch (err: any) {
        errors.push({ file: file.filePath, error: err.message || String(err) });
      }
    }

    return { indexedCount, errors };
  }
}
