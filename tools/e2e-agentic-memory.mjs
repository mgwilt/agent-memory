#!/usr/bin/env node
import { spawn } from 'node:child_process';
import { createWriteStream } from 'node:fs';
import { mkdir, writeFile } from 'node:fs/promises';
import net from 'node:net';
import path from 'node:path';
import process from 'node:process';

import { createOpenAICompatible } from '@ai-sdk/openai-compatible';
import { generateText } from 'ai';

const REPO_ROOT = path.resolve(new URL('..', import.meta.url).pathname);
const DIRECT_DEPENDENCY_POLICY = [
  { name: 'ai', version: '6.0.208', publishedAt: '2026-06-18T01:01:39.789Z' },
  {
    name: '@ai-sdk/openai-compatible',
    version: '2.0.51',
    publishedAt: '2026-06-16T22:04:54.676Z',
  },
  { name: 'zod', version: '4.4.3', publishedAt: '2026-05-04T07:06:40.819Z' },
];

const EXPECTED_ROUTES = [
  'POST /v1/memory/chunks',
  'GET /v1/memory/chunks/{chunk_id}',
  'PATCH /v1/memory/chunks/{chunk_id}',
  'DELETE /v1/memory/chunks/{chunk_id}',
  'POST /v1/memory/retrieve',
  'POST /v1/memory/retrieve/stream',
  'POST /v1/memory/practice',
  'POST /v1/memory/associate',
  'PUT /v1/memory/buffers/{buffer_name}',
  'POST /v1/rules/evaluate',
  'GET /healthz',
  'GET /readyz',
  'GET /metrics',
];

const REQUIRED_MEMORIES = {
  'mem-name': {
    chunk_type: 'fact',
    topic: 'identity',
    subject: 'eli',
    detail: 'name-eli',
  },
  'mem-preference': {
    chunk_type: 'fact',
    topic: 'preference',
    subject: 'eli',
    detail: 'strong-black-coffee',
  },
  'mem-project': {
    chunk_type: 'fact',
    topic: 'project',
    subject: 'eli',
    detail: 'agent-memory-cli',
  },
};

const options = parseArgs(process.argv.slice(2));
const runStartedAt = new Date();
const runStamp = runStartedAt.toISOString().replace(/[:.]/g, '-');
const runDir = path.resolve(options.artifactsDir, runStamp);
const transcript = [];
const checks = [];
const httpExchanges = [];
const coveredRoutes = new Set();
let spawnedApi = null;
let apiBaseUrl = options.apiUrl;

await main().catch(async (error) => {
  await stopApi(spawnedApi);
  spawnedApi = null;
  const message = error instanceof Error ? error.message : String(error);
  log(`FAILED ${message}`);
  const failure = {
    status: 'failed',
    startedAt: runStartedAt.toISOString(),
    failedAt: new Date().toISOString(),
    message,
    stack: error instanceof Error ? error.stack : undefined,
    artifactsDir: runDir,
    checks,
    coveredRoutes: [...coveredRoutes].sort(),
  };
  try {
    await mkdir(runDir, { recursive: true });
    await writeJson('failure.json', failure);
    await writeText('transcript.md', transcript.join('\n'));
  } catch (writeError) {
    console.error(`failed to write failure artifacts: ${formatError(writeError)}`);
  }
  console.error(formatError(error));
  process.exitCode = 1;
});

async function main() {
  await mkdir(runDir, { recursive: true });
  log(`# Agentic memory E2E`);
  log(`artifacts: ${runDir}`);
  log(`LM Studio: ${options.lmstudioUrl}`);
  log(`model: ${options.model}`);

  await writeJson('dependency-policy.json', {
    minimumReleaseAgeMinutes: 2880,
    directDependencies: DIRECT_DEPENDENCY_POLICY,
    pnpmConfig: {
      saveExact: true,
      minimumReleaseAge: 2880,
    },
  });

  if (!options.skipRustTest) {
    log(`running Rust HTTP/formula integration test`);
    await runCommand(
      'cargo',
      ['test', '-p', 'nestor-api', '--test', 'http_end_to_end', '--', '--nocapture'],
      'rust-test.log',
    );
    pass('rust integration test passed');
  }

  await preflightLmStudio();
  pass('LM Studio model is reachable');

  if (!apiBaseUrl) {
    const port = await getFreePort();
    apiBaseUrl = `http://127.0.0.1:${port}`;
    spawnedApi = await startApi(port);
  } else {
    log(`using existing Nestor API: ${apiBaseUrl}`);
    await waitForApi(apiBaseUrl, null, 15_000);
  }
  pass('Nestor API is reachable');

  const agentId = `agentic-${Date.now()}`;
  const lmstudio = createOpenAICompatible({
    name: 'lmstudio',
    baseURL: options.lmstudioUrl,
  });
  const model = lmstudio(options.model);

  const planPrompt = [
    'Return only a JSON object. Do not use markdown.',
    'Build a memory-write plan for this exact user statement:',
    '"My name is Eli. I am validating the agent-memory CLI. I prefer strong black coffee before debugging."',
    '',
    'Use exactly these chunk ids and normalized values:',
    '- goal chunk id ctx-goal, chunk_type goal, task answer-memory-question, owner eli',
    '- memory chunk id mem-name, chunk_type fact, topic identity, subject eli, detail name-eli',
    '- memory chunk id mem-preference, chunk_type fact, topic preference, subject eli, detail strong-black-coffee',
    '- memory chunk id mem-project, chunk_type fact, topic project, subject eli, detail agent-memory-cli',
    '',
    'Schema:',
    '{"goal":{"chunk_id":"ctx-goal","chunk_type":"goal","task":"answer-memory-question","owner":"eli"},"memories":[{"chunk_id":"mem-name","chunk_type":"fact","topic":"identity","subject":"eli","detail":"name-eli"}]}',
  ].join('\n');

  log(`asking local model to produce the memory plan`);
  const planText = await inferText(model, planPrompt);
  await writeText('lm-plan.raw.txt', planText);
  const plan = validateMemoryPlan(extractJson(planText));
  await writeJson('lm-plan.json', plan);
  pass('local model produced a valid memory-write plan');

  await exerciseApiWorkflow(agentId, plan, model);

  const missingRoutes = EXPECTED_ROUTES.filter((route) => !coveredRoutes.has(route));
  if (missingRoutes.length > 0) {
    throw new Error(`missing endpoint coverage: ${missingRoutes.join(', ')}`);
  }
  pass('all API endpoints were covered');
  await stopApi(spawnedApi);
  spawnedApi = null;

  const summary = {
    status: 'passed',
    startedAt: runStartedAt.toISOString(),
    finishedAt: new Date().toISOString(),
    apiBaseUrl,
    lmstudioBaseUrl: options.lmstudioUrl,
    model: options.model,
    artifactsDir: runDir,
    checks,
    coveredRoutes: [...coveredRoutes].sort(),
    dependencyPolicy: {
      minimumReleaseAgeMinutes: 2880,
      directDependencies: DIRECT_DEPENDENCY_POLICY,
    },
  };
  await writeJson('summary.json', summary);
  await writeJson('http-exchanges.json', httpExchanges);

  log(`passed ${checks.length} checks`);
  log(`summary: ${path.join(runDir, 'summary.json')}`);
  await writeText('transcript.md', transcript.join('\n'));
}

async function exerciseApiWorkflow(agentId, plan, model) {
  const health = await request('GET', '/healthz', null, 'GET /healthz');
  assertEqual(health.status, 'pass', 'healthz status');

  const ready = await request('GET', '/readyz', null, 'GET /readyz');
  assertEqual(ready.status, 'warn', 'readyz status');

  await request(
    'POST',
    '/v1/memory/chunks',
    {
      agent_id: agentId,
      chunk_id: plan.goal.chunk_id,
      chunk_type: plan.goal.chunk_type,
      now_ms: 1_000,
      slots: {
        task: symbol(plan.goal.task),
        owner: symbol(plan.goal.owner),
      },
    },
    'POST /v1/memory/chunks',
  );

  for (const memory of plan.memories) {
    await request(
      'POST',
      '/v1/memory/chunks',
      {
        agent_id: agentId,
        chunk_id: memory.chunk_id,
        chunk_type: memory.chunk_type,
        now_ms: 1_000,
        slots: {
          topic: symbol(memory.topic),
          subject: symbol(memory.subject),
          detail: symbol(memory.detail),
        },
      },
      'POST /v1/memory/chunks',
    );
  }

  await request(
    'POST',
    '/v1/memory/chunks',
    {
      agent_id: agentId,
      chunk_id: 'delete-me',
      chunk_type: 'fact',
      now_ms: 1_000,
      slots: { topic: symbol('temporary') },
    },
    'POST /v1/memory/chunks',
  );
  pass('chunks were created from the local model plan');

  const preference = await request(
    'GET',
    `/v1/memory/chunks/mem-preference?agent_id=${encodeURIComponent(agentId)}`,
    null,
    'GET /v1/memory/chunks/{chunk_id}',
  );
  assertEqual(preference.slots.detail.value, 'strong-black-coffee', 'created memory detail');
  pass('created memory was read back mechanically');

  const project = REQUIRED_MEMORIES['mem-project'];
  const patched = await request(
    'PATCH',
    '/v1/memory/chunks/mem-project',
    {
      agent_id: agentId,
      expected_version: 1,
      slots: {
        topic: symbol(project.topic),
        subject: symbol(project.subject),
        detail: symbol(project.detail),
        verified: { type: 'bool', value: true },
      },
    },
    'PATCH /v1/memory/chunks/{chunk_id}',
  );
  assertEqual(patched.slots.verified.value, true, 'patched slot');
  pass('patch endpoint updated chunk slots');

  const practice = await request(
    'POST',
    '/v1/memory/practice',
    {
      agent_id: agentId,
      chunk_id: 'mem-preference',
      event_id: `practice-${agentId}-preference-1`,
      kind: 'retrieve',
      weight: 2.0,
      occurred_at_ms: 1_500,
    },
    'POST /v1/memory/practice',
  );
  assertEqual(practice.weight, 2.0, 'practice weight');

  const association = await request(
    'POST',
    '/v1/memory/associate',
    {
      agent_id: agentId,
      src_chunk_id: 'ctx-goal',
      dst_chunk_id: 'mem-preference',
      source: 'goal',
      strength: 1.25,
      fan: 1,
      updated_at_ms: 2_000,
    },
    'POST /v1/memory/associate',
  );
  assertEqual(association.strength, 1.25, 'association strength');

  const buffer = await request(
    'PUT',
    '/v1/memory/buffers/goal',
    {
      agent_id: agentId,
      chunk_id: 'ctx-goal',
      set_at_ms: 2_500,
    },
    'PUT /v1/memory/buffers/{buffer_name}',
  );
  assertEqual(buffer.chunk_id, 'ctx-goal', 'goal buffer chunk');
  pass('practice, association, and buffer endpoints accepted memory state');

  const retrievalRequest = {
    agent_id: agentId,
    chunk_type: 'fact',
    cue_slots: [{ key: 'topic', value: symbol('preference') }],
    context_chunk_ids: ['ctx-goal'],
    candidate_limit: 10,
    result_limit: 3,
    activation_threshold: -10.0,
    noise_s: 0.0,
    partial_matching: true,
    return_diagnostics: true,
    deterministic_seed: 42,
    commit_on_hit: true,
    now_ms: 11_000,
  };

  const streamRetrieval = await request(
    'POST',
    '/v1/memory/retrieve/stream',
    retrievalRequest,
    'POST /v1/memory/retrieve/stream',
  );
  assertRetrievalHit(streamRetrieval, 'stream retrieval');

  const retrieval = await request(
    'POST',
    '/v1/memory/retrieve',
    retrievalRequest,
    'POST /v1/memory/retrieve',
  );
  assertRetrievalHit(retrieval, 'retrieval');
  const formula = assertFormula(retrieval);
  await writeJson('formula-validation.json', formula);
  pass('Rust retrieval formula diagnostics match independent calculation');

  const rule = await request(
    'POST',
    '/v1/rules/evaluate',
    {
      agent_id: agentId,
      retrieved_chunk_id: 'mem-preference',
      rules: [
        {
          rule_id: 'answer-with-retrieved-preference',
          name: 'answer with retrieved preference',
          utility: 2.0,
          conditions: [{ buffer: 'goal', chunk_type: 'goal' }],
          retrieved_chunk: {
            chunk_type: 'fact',
            slots: [{ key: 'topic', value: symbol('preference') }],
          },
        },
      ],
    },
    'POST /v1/rules/evaluate',
  );
  assertEqual(
    rule.selected.rule_id,
    'answer-with-retrieved-preference',
    'selected production rule',
  );
  pass('rule endpoint selected the retrieved-memory production');

  const answerPrompt = [
    'Return only JSON. Do not use markdown.',
    'You are an agent answering a user after retrieving Nestor memory.',
    'User asks: "What should I prepare for Eli before debugging?"',
    `Retrieved memory JSON: ${JSON.stringify(retrieval.results[0])}`,
    `Selected production rule: ${JSON.stringify(rule.selected)}`,
    'Return schema: {"answer":"one sentence","used_chunk_ids":["mem-preference"],"rule_id":"answer-with-retrieved-preference"}',
    'The answer must mention strong black coffee.',
  ].join('\n');

  log(`asking local model to answer from retrieved memory`);
  const answerText = await inferText(model, answerPrompt);
  await writeText('lm-answer.raw.txt', answerText);
  const answer = validateAnswer(extractJson(answerText));
  await writeJson('lm-answer.json', answer);
  pass('local model answered using the retrieved memory');

  const deleted = await request(
    'DELETE',
    `/v1/memory/chunks/delete-me?agent_id=${encodeURIComponent(agentId)}`,
    null,
    'DELETE /v1/memory/chunks/{chunk_id}',
  );
  assertEqual(deleted.deleted, true, 'delete response');

  const metrics = await requestText('GET', '/metrics', 'GET /metrics');
  if (!metrics.includes('nestor_memory_retrieval_hits_total 2')) {
    throw new Error('metrics did not report two retrieval hits');
  }
  if (!metrics.includes('nestor_memory_candidates_examined 1')) {
    throw new Error('metrics did not report one candidate examined');
  }
  pass('metrics endpoint recorded retrieval behavior');
}

async function preflightLmStudio() {
  const modelsUrl = joinUrl(options.lmstudioUrl, 'models');
  log(`checking LM Studio models at ${modelsUrl}`);
  const response = await fetchJson(modelsUrl, { timeoutMs: 10_000 });
  const ids = Array.isArray(response.data)
    ? response.data.map((entry) => entry && entry.id).filter(Boolean)
    : [];
  await writeJson('lmstudio-models.json', response);
  if (!ids.includes(options.model)) {
    throw new Error(
      `LM Studio model ${options.model} was not listed by ${modelsUrl}. Available: ${ids.join(', ')}`,
    );
  }
}

async function inferText(model, prompt) {
  const result = await generateText({
    model,
    prompt,
    temperature: 0,
    maxRetries: 1,
  });
  return result.text;
}

async function startApi(port) {
  const apiLog = createWriteStream(path.join(runDir, 'api-server.log'), { flags: 'a' });
  log(`starting Nestor API on ${apiBaseUrl}`);
  const child = spawn('cargo', ['run', '-p', 'nestor-api', '--', 'serve'], {
    cwd: REPO_ROOT,
    env: {
      ...process.env,
      NESTOR_API_BIND_ADDR: `127.0.0.1:${port}`,
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  let exited = false;
  child.stdout.on('data', (chunk) => {
    process.stdout.write(chunk);
    apiLog.write(chunk);
  });
  child.stderr.on('data', (chunk) => {
    process.stderr.write(chunk);
    apiLog.write(chunk);
  });
  child.on('exit', (code, signal) => {
    exited = true;
    apiLog.write(`\n[api exited code=${code} signal=${signal}]\n`);
  });

  try {
    await waitForApi(apiBaseUrl, () => exited, 120_000);
    return { child, apiLog, exited: () => exited };
  } catch (error) {
    await stopApi({ child, apiLog, exited: () => exited });
    throw error;
  }
}

async function waitForApi(baseUrl, exited, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (exited && exited()) {
      throw new Error('Nestor API process exited before becoming ready');
    }
    try {
      const response = await fetchJson(joinUrl(baseUrl, 'healthz'), { timeoutMs: 1_000 });
      if (response.status === 'pass') {
        return;
      }
    } catch {
      await delay(500);
    }
  }
  throw new Error(`Nestor API did not become ready within ${timeoutMs}ms`);
}

async function stopApi(api) {
  if (!api || api.exited()) {
    return;
  }
  api.child.kill('SIGTERM');
  const stopped = await Promise.race([
    new Promise((resolve) => api.child.once('exit', () => resolve(true))),
    delay(5_000).then(() => false),
  ]);
  if (!stopped && !api.exited()) {
    api.child.kill('SIGKILL');
  }
  api.apiLog.end();
}

async function request(method, routePath, body, manifestRoute) {
  const text = await requestRaw(method, routePath, body, manifestRoute);
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`response from ${method} ${routePath} was not JSON: ${formatError(error)}`);
  }
}

async function requestText(method, routePath, manifestRoute) {
  return requestRaw(method, routePath, null, manifestRoute);
}

async function requestRaw(method, routePath, body, manifestRoute) {
  const url = joinUrl(apiBaseUrl, routePath);
  const headers = body === null ? {} : { 'content-type': 'application/json' };
  const response = await fetch(url, {
    method,
    headers,
    body: body === null ? undefined : JSON.stringify(body),
  });
  const text = await response.text();
  httpExchanges.push({
    route: manifestRoute,
    method,
    url,
    status: response.status,
    request: body,
    response: parseJsonOrText(text),
  });
  coveredRoutes.add(manifestRoute);
  if (!response.ok) {
    throw new Error(`${method} ${routePath} returned ${response.status}: ${text}`);
  }
  return text;
}

async function fetchJson(url, { timeoutMs }) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetch(url, { signal: controller.signal });
    const text = await response.text();
    if (!response.ok) {
      throw new Error(`${url} returned ${response.status}: ${text}`);
    }
    return JSON.parse(text);
  } finally {
    clearTimeout(timeout);
  }
}

async function runCommand(command, args, artifactName) {
  const logPath = path.join(runDir, artifactName);
  const file = createWriteStream(logPath, { flags: 'a' });
  log(`$ ${[command, ...args].join(' ')}`);
  await new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: REPO_ROOT,
      env: process.env,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    child.stdout.on('data', (chunk) => {
      process.stdout.write(chunk);
      file.write(chunk);
    });
    child.stderr.on('data', (chunk) => {
      process.stderr.write(chunk);
      file.write(chunk);
    });
    child.on('error', reject);
    child.on('exit', (code, signal) => {
      file.end();
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`${command} exited with code ${code} signal ${signal}`));
      }
    });
  });
}

function validateMemoryPlan(value) {
  const goal = value.goal;
  if (!goal || goal.chunk_id !== 'ctx-goal' || goal.chunk_type !== 'goal') {
    throw new Error('model plan did not include the required goal chunk');
  }
  assertEqual(goal.task, 'answer-memory-question', 'planned goal task');
  assertEqual(goal.owner, 'eli', 'planned goal owner');

  if (!Array.isArray(value.memories)) {
    throw new Error('model plan did not include a memories array');
  }
  const byId = new Map(value.memories.map((memory) => [memory.chunk_id, memory]));
  const memories = [];
  for (const [chunkId, expected] of Object.entries(REQUIRED_MEMORIES)) {
    const actual = byId.get(chunkId);
    if (!actual) {
      throw new Error(`model plan omitted ${chunkId}`);
    }
    for (const [key, expectedValue] of Object.entries(expected)) {
      assertEqual(actual[key], expectedValue, `model plan ${chunkId}.${key}`);
    }
    memories.push({
      chunk_id: chunkId,
      ...expected,
    });
  }
  return {
    goal: {
      chunk_id: 'ctx-goal',
      chunk_type: 'goal',
      task: 'answer-memory-question',
      owner: 'eli',
    },
    memories,
  };
}

function validateAnswer(value) {
  if (typeof value.answer !== 'string') {
    throw new Error('model answer did not include an answer string');
  }
  if (!value.answer.toLowerCase().includes('strong black coffee')) {
    throw new Error(`model answer did not use retrieved preference: ${value.answer}`);
  }
  if (!Array.isArray(value.used_chunk_ids) || !value.used_chunk_ids.includes('mem-preference')) {
    throw new Error('model answer did not cite mem-preference');
  }
  assertEqual(value.rule_id, 'answer-with-retrieved-preference', 'answer rule id');
  return value;
}

function extractJson(text) {
  const withoutThink = text.replace(/<think>[\s\S]*?<\/think>/gi, '').trim();
  const start = withoutThink.indexOf('{');
  const end = withoutThink.lastIndexOf('}');
  if (start < 0 || end < start) {
    throw new Error(`model response did not contain a JSON object: ${text}`);
  }
  const candidate = withoutThink.slice(start, end + 1);
  try {
    return JSON.parse(candidate);
  } catch (error) {
    throw new Error(`failed to parse model JSON: ${formatError(error)}\n${candidate}`);
  }
}

function assertRetrievalHit(body, label) {
  assertEqual(body.status, 'hit', `${label} status`);
  assertEqual(body.results[0].chunk_id, 'mem-preference', `${label} chunk`);
  assertEqual(body.diagnostics.candidates_examined, 1, `${label} candidates`);
}

function assertFormula(body) {
  const result = body.results[0];
  const actual = {
    baseLevel: result.components.base_level,
    spreading: result.components.spreading,
    partialMatch: result.components.partial_match,
    noise: result.components.noise,
    activation: result.activation,
    probability: result.retrieval_probability,
    latencyMs: result.predicted_latency_ms,
  };
  const expected = {
    baseLevel: Math.log(10 ** -0.5 + 2.0 * 9.5 ** -0.5),
    spreading: 1.25,
    partialMatch: 0.0,
    noise: 0.0,
  };
  expected.activation =
    expected.baseLevel + expected.spreading + expected.partialMatch + expected.noise;
  expected.probability = 1.0;
  expected.latencyMs = 350.0 * Math.exp(-expected.activation);

  assertClose(actual.baseLevel, expected.baseLevel, 1e-12, 'base level');
  assertClose(actual.spreading, expected.spreading, 1e-12, 'spreading');
  assertClose(actual.partialMatch, expected.partialMatch, 1e-12, 'partial match');
  assertClose(actual.noise, expected.noise, 1e-12, 'noise');
  assertClose(actual.activation, expected.activation, 1e-12, 'activation');
  assertClose(actual.probability, expected.probability, 1e-12, 'probability');
  assertClose(actual.latencyMs, expected.latencyMs, 1e-9, 'latency');
  assertEqual(result.passes_threshold, true, 'passes threshold');
  return { actual, expected };
}

function assertClose(actual, expected, tolerance, label) {
  if (typeof actual !== 'number' || !Number.isFinite(actual)) {
    throw new Error(`${label} was not a finite number: ${actual}`);
  }
  if (Math.abs(actual - expected) > tolerance) {
    throw new Error(
      `${label} mismatch: actual=${actual}, expected=${expected}, tolerance=${tolerance}`,
    );
  }
}

function assertEqual(actual, expected, label) {
  if (actual !== expected) {
    throw new Error(`${label} mismatch: actual=${JSON.stringify(actual)}, expected=${JSON.stringify(expected)}`);
  }
}

function symbol(value) {
  return { type: 'symbol', value };
}

function pass(name) {
  checks.push({ name, status: 'passed', at: new Date().toISOString() });
  log(`PASS ${name}`);
}

function log(message) {
  const line = `[${new Date().toISOString()}] ${message}`;
  transcript.push(line);
  console.log(line);
}

async function writeJson(name, value) {
  await writeText(name, `${JSON.stringify(value, null, 2)}\n`);
}

async function writeText(name, value) {
  await writeFile(path.join(runDir, name), value);
}

function parseJsonOrText(text) {
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

function joinUrl(base, routePath) {
  const normalizedBase = base.endsWith('/') ? base : `${base}/`;
  return new URL(routePath.replace(/^\//, ''), normalizedBase).toString();
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function getFreePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () => {
      const address = server.address();
      server.close(() => {
        if (!address || typeof address === 'string') {
          reject(new Error('failed to allocate a TCP port'));
        } else {
          resolve(address.port);
        }
      });
    });
  });
}

function formatError(error) {
  return error instanceof Error ? error.message : String(error);
}

function parseArgs(argv) {
  const parsed = {
    lmstudioUrl: process.env.LMSTUDIO_BASE_URL || 'http://localhost:1234/v1',
    model: process.env.LMSTUDIO_MODEL || 'qwen/qwen3.6-27b',
    apiUrl: process.env.NESTOR_E2E_API_URL || '',
    artifactsDir:
      process.env.NESTOR_E2E_ARTIFACTS_DIR || path.join(REPO_ROOT, 'artifacts/e2e-agentic-memory'),
    skipRustTest: process.env.NESTOR_E2E_SKIP_RUST_TEST === '1',
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--lmstudio-url') {
      parsed.lmstudioUrl = requireValue(argv, (index += 1), arg);
    } else if (arg === '--model') {
      parsed.model = requireValue(argv, (index += 1), arg);
    } else if (arg === '--api-url') {
      parsed.apiUrl = requireValue(argv, (index += 1), arg);
    } else if (arg === '--artifacts-dir') {
      parsed.artifactsDir = path.resolve(requireValue(argv, (index += 1), arg));
    } else if (arg === '--skip-rust-test') {
      parsed.skipRustTest = true;
    } else if (arg === '--help' || arg === '-h') {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }

  parsed.lmstudioUrl = parsed.lmstudioUrl.replace(/\/$/, '');
  parsed.apiUrl = parsed.apiUrl ? parsed.apiUrl.replace(/\/$/, '') : '';
  return parsed;
}

function requireValue(argv, index, flag) {
  const value = argv[index];
  if (!value || value.startsWith('--')) {
    throw new Error(`${flag} requires a value`);
  }
  return value;
}

function printHelp() {
  console.log(`Usage: pnpm e2e:agentic [options]

Options:
  --lmstudio-url <url>   LM Studio OpenAI-compatible base URL (default: http://localhost:1234/v1)
  --model <id>           LM Studio model id (default: qwen/qwen3.6-27b)
  --api-url <url>        Use an already running Nestor API instead of starting cargo run
  --artifacts-dir <dir>  Artifact root (default: artifacts/e2e-agentic-memory)
  --skip-rust-test       Skip the deterministic Rust integration test
`);
}

process.on('exit', () => {
  if (spawnedApi && !spawnedApi.exited()) {
    spawnedApi.child.kill('SIGTERM');
  }
});

process.on('SIGINT', async () => {
  await stopApi(spawnedApi);
  process.exit(130);
});

process.on('SIGTERM', async () => {
  await stopApi(spawnedApi);
  process.exit(143);
});
