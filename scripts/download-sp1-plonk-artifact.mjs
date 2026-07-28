#!/usr/bin/env node

import fs from "node:fs/promises";
import https from "node:https";
import path from "node:path";
import process from "node:process";

const [, , sourceUrl, outputPath, expectedSizeText] = process.argv;
const expectedSize = Number(expectedSizeText);

if (!sourceUrl || !outputPath || !Number.isSafeInteger(expectedSize) || expectedSize <= 0) {
  console.error("usage: download-sp1-plonk-artifact.mjs SOURCE_URL OUTPUT_PATH EXPECTED_SIZE");
  process.exit(2);
}

const chunkSize = 16 * 1024 * 1024;
const concurrency = 16;
const maximumAttempts = 20;
const statePath = `${outputPath}.ranges.json`;
const stateTemporaryPath = `${statePath}.tmp`;
const lockPath = `${outputPath}.lock`;
const chunkCount = Math.ceil(expectedSize / chunkSize);
const agent = new https.Agent({ keepAlive: true, maxSockets: concurrency });

let state;
let output;
let lockHeld = false;

try {
  await fs.mkdir(path.dirname(outputPath), { recursive: true });
  try {
    await fs.mkdir(lockPath);
    lockHeld = true;
  } catch (error) {
    if (error.code === "EEXIST") {
      throw new Error(`another artifact download holds ${lockPath}`);
    }
    throw error;
  }

  state = await loadOrCreateState();
  output = await fs.open(outputPath, "r+");

  let cursor = 0;
  let completedCount = state.completed.filter(Boolean).length;
  let saveChain = Promise.resolve();

  const nextChunk = () => {
    while (cursor < chunkCount && state.completed[cursor]) {
      cursor += 1;
    }
    if (cursor >= chunkCount) {
      return undefined;
    }
    const index = cursor;
    cursor += 1;
    return index;
  };

  const saveState = () => {
    saveChain = saveChain.then(async () => {
      await fs.writeFile(stateTemporaryPath, `${JSON.stringify(state)}\n`);
      await fs.rename(stateTemporaryPath, statePath);
    });
    return saveChain;
  };

  const worker = async () => {
    for (let index = nextChunk(); index !== undefined; index = nextChunk()) {
      const start = index * chunkSize;
      const end = Math.min(expectedSize - 1, start + chunkSize - 1);
      const bytes = await downloadRangeWithRetry(start, end);
      await writeExact(output, bytes, start);
      await output.sync();
      state.completed[index] = true;
      completedCount += 1;
      await saveState();
      const percent = ((completedCount / chunkCount) * 100).toFixed(1);
      console.log(`downloaded chunk ${completedCount}/${chunkCount} (${percent}%)`);
    }
  };

  await Promise.all(Array.from({ length: concurrency }, worker));
  await saveChain;
  await output.sync();
  await output.close();
  output = undefined;
  await fs.unlink(statePath);
  console.log(`download complete: ${outputPath}`);
} finally {
  agent.destroy();
  try {
    if (output) {
      await output.close();
    }
  } finally {
    if (lockHeld) {
      await fs.rmdir(lockPath);
    }
  }
}

async function loadOrCreateState() {
  try {
    const parsed = JSON.parse(await fs.readFile(statePath, "utf8"));
    if (
      parsed.sourceUrl !== sourceUrl ||
      parsed.expectedSize !== expectedSize ||
      parsed.chunkSize !== chunkSize ||
      !Array.isArray(parsed.completed) ||
      parsed.completed.length !== chunkCount ||
      !parsed.completed.every((value) => typeof value === "boolean")
    ) {
      throw new Error("download state does not match the pinned artifact");
    }
    const metadata = await fs.stat(outputPath);
    if (metadata.size !== expectedSize) {
      throw new Error("partial artifact has the wrong allocated size");
    }
    return parsed;
  } catch (error) {
    if (error.code !== "ENOENT") {
      throw error;
    }
  }

  const file = await fs.open(outputPath, "w+");
  await file.truncate(expectedSize);
  await file.close();
  const created = {
    sourceUrl,
    expectedSize,
    chunkSize,
    completed: Array(chunkCount).fill(false),
  };
  await fs.writeFile(statePath, `${JSON.stringify(created)}\n`);
  return created;
}

async function downloadRangeWithRetry(start, end) {
  let lastError;
  for (let attempt = 1; attempt <= maximumAttempts; attempt += 1) {
    try {
      return await downloadRange(start, end);
    } catch (error) {
      lastError = error;
      const delay = Math.min(5_000, attempt * 250);
      console.error(`range ${start}-${end} attempt ${attempt} failed: ${error.message}`);
      await new Promise((resolve) => setTimeout(resolve, delay));
    }
  }
  throw lastError;
}

function downloadRange(start, end) {
  const expectedLength = end - start + 1;
  const expectedContentRange = `bytes ${start}-${end}/${expectedSize}`;

  return new Promise((resolve, reject) => {
    const request = https.get(
      sourceUrl,
      {
        agent,
        headers: {
          "Accept-Encoding": "identity",
          Range: `bytes=${start}-${end}`,
        },
      },
      (response) => {
        if (
          response.statusCode !== 206 ||
          response.headers["content-range"] !== expectedContentRange
        ) {
          response.resume();
          reject(
            new Error(
              `unexpected range response: status=${response.statusCode} content-range=${response.headers["content-range"]}`,
            ),
          );
          return;
        }

        const chunks = [];
        let received = 0;
        response.on("data", (chunk) => {
          chunks.push(chunk);
          received += chunk.length;
        });
        response.on("end", () => {
          clearTimeout(deadline);
          if (received !== expectedLength) {
            reject(
              new Error(`short range response: expected ${expectedLength}, found ${received}`),
            );
            return;
          }
          resolve(Buffer.concat(chunks, received));
        });
        response.on("error", (error) => {
          clearTimeout(deadline);
          reject(error);
        });
      },
    );

    const deadline = setTimeout(() => {
      request.destroy(new Error("range request exceeded its 60-second deadline"));
    }, 60_000);
    request.on("error", (error) => {
      clearTimeout(deadline);
      reject(error);
    });
  });
}

async function writeExact(file, bytes, position) {
  let written = 0;
  while (written < bytes.length) {
    const result = await file.write(bytes, written, bytes.length - written, position + written);
    if (result.bytesWritten === 0) {
      throw new Error("artifact write made no progress");
    }
    written += result.bytesWritten;
  }
}
