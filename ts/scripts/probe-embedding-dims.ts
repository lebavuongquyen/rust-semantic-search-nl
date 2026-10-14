import { GoogleAiStudioProvider } from "../google-ai-studio-provider";
import * as fs from "fs";
import * as path from "path";

// Read API key from .env if present, or process.env
function getApiKey(): string | undefined {
  if (process.env.GOOGLE_AI_STUDIO_API_KEY) {
    return process.env.GOOGLE_AI_STUDIO_API_KEY;
  }
  if (process.env.GEMINI_API_KEY) {
    return process.env.GEMINI_API_KEY;
  }
  const envPath = path.resolve(__dirname, "../../.env");
  if (fs.existsSync(envPath)) {
    const lines = fs.readFileSync(envPath, "utf-8").split("\n");
    for (const line of lines) {
      const match = line.match(
        /^\s*(?:GOOGLE_AI_STUDIO_API_KEY|GEMINI_API_KEY)\s*=\s*(["']?)(.*?)\1\s*$/,
      );
      if (match && match[2]) {
        return match[2].trim();
      }
    }
  }
  return undefined;
}

function cosineSimilarity(a: number[], b: number[]): number {
  let dot = 0;
  let normA = 0;
  let normB = 0;
  for (let i = 0; i < a.length; i++) {
    dot += a[i] * b[i];
    normA += a[i] * a[i];
    normB += b[i] * b[i];
  }
  if (normA === 0 || normB === 0) return 0;
  return dot / (Math.sqrt(normA) * Math.sqrt(normB));
}

async function probe() {
  console.log("=================================================");
  console.log("PROBING GOOGLE AI STUDIO EMBEDDING MODELS");
  console.log("=================================================");

  const apiKey = getApiKey();
  if (!apiKey) {
    console.log("[INFO] No API key detected in env or .env file.");
    console.log(
      "[INFO] Testing with mock fallback probe to verify probe pipeline.",
    );
    const mockDim = 768;
    console.log(
      `[MOCK PROBE RESULT] Default expected embedding dimension: ${mockDim}`,
    );
    return;
  }

  const provider = new GoogleAiStudioProvider({ apiKey });

  console.log("\nListing all available models from Google AI Studio...");
  try {
    const allModels = await provider.listModels();
    console.log(`Discovered ${allModels.length} models.`);
    const embeddingModels = allModels.filter((m) =>
      m.capabilities.has("text_embedding"),
    );
    console.log(`Found ${embeddingModels.length} embedding models:`);
    for (const m of embeddingModels) {
      console.log(
        ` - ${m.id} (${m.displayName}) methods: ${m.metadata?.methods}`,
      );
    }

    const candidateModels =
      embeddingModels.length > 0
        ? embeddingModels.map((m) => m.id)
        : [
            "text-embedding-004",
            "embedding-001",
            "gemini-embedding-1",
            "gemini-embedding-2",
          ];

    for (const model of candidateModels) {
      try {
        console.log(`\nTesting model: ${model}...`);
        const res = await provider.embed({
          model,
          inputs: ["tìm hóa đơn", "invoice"],
        });
        console.log(`-> SUCCESS! Dimensions: ${res.dimensions}`);
        const sim = cosineSimilarity(
          res.vectors[0] as number[],
          res.vectors[1] as number[],
        );
        console.log(
          `-> Cross-lingual cosine similarity ('tìm hóa đơn' <-> 'invoice'): ${sim.toFixed(4)}`,
        );
      } catch (err: any) {
        console.log(`-> Model ${model} failed: ${err.message || err}`);
      }
    }
  } catch (err: any) {
    console.log(`Failed to list models: ${err.message || err}`);
  }
}

probe().catch(console.error);
