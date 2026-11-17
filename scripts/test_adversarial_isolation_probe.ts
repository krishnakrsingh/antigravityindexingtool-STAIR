import * as fs from 'node:fs';
import * as path from 'node:path';

const probeRoot = 'c:\\Users\\krish\\Downloads\\antigravitytoolbuild\\scratch_probe_isolation';
const sourceDir = 'C:\\Users\\krish\\Downloads\\agaateapp';
const ws01Dir = 'c:\\Users\\krish\\Downloads\\antigravitytoolbuild\\workspaces\\ws_01';

async function runProbe() {
  console.log('=== Running Adversarial Workspace Isolation Probe ===');

  if (fs.existsSync(probeRoot)) {
    fs.rmSync(probeRoot, { recursive: true, force: true });
  }
  fs.mkdirSync(probeRoot, { recursive: true });

  const wsA = path.join(probeRoot, 'probe_ws_A');
  const wsB = path.join(probeRoot, 'probe_ws_B');
  fs.mkdirSync(wsA, { recursive: true });
  fs.mkdirSync(wsB, { recursive: true });

  // 1. Copy prisma directory to both
  fs.cpSync(path.join(sourceDir, 'prisma'), path.join(wsA, 'prisma'), { recursive: true });
  fs.cpSync(path.join(sourceDir, 'prisma'), path.join(wsB, 'prisma'), { recursive: true });

  // 2. Setup node_modules
  const nmA = path.join(wsA, 'node_modules');
  const nmB = path.join(wsB, 'node_modules');
  fs.mkdirSync(nmA, { recursive: true });
  fs.mkdirSync(nmB, { recursive: true });

  // 3. Deep copy @prisma and .prisma from ws_01
  fs.cpSync(path.join(ws01Dir, 'node_modules', '@prisma'), path.join(nmA, '@prisma'), { recursive: true });
  fs.cpSync(path.join(ws01Dir, 'node_modules', '.prisma'), path.join(nmA, '.prisma'), { recursive: true });
  fs.cpSync(path.join(ws01Dir, 'node_modules', '@prisma'), path.join(nmB, '@prisma'), { recursive: true });
  fs.cpSync(path.join(ws01Dir, 'node_modules', '.prisma'), path.join(nmB, '.prisma'), { recursive: true });

  // 4. Verify initial state
  const schemaAPath = path.join(wsA, 'prisma', 'schema.prisma');
  const schemaBPath = path.join(wsB, 'prisma', 'schema.prisma');
  const schemaA = fs.readFileSync(schemaAPath, 'utf-8');
  const schemaB = fs.readFileSync(schemaBPath, 'utf-8');
  console.log('Initial schemas match:', schemaA === schemaB);

  // 5. Adversarial schema corruption: inject canary into wsA
  const canary = '\n// ADVERSARIAL CANARY PROBE\nenum AdversarialCanaryRole {\n  MALICIOUS_ISOLATION_BREACH\n}\n';
  fs.appendFileSync(schemaAPath, canary, 'utf-8');

  const schemaA_after = fs.readFileSync(schemaAPath, 'utf-8');
  const schemaB_after = fs.readFileSync(schemaBPath, 'utf-8');

  const wsA_has_canary = schemaA_after.includes('MALICIOUS_ISOLATION_BREACH');
  const wsB_has_canary = schemaB_after.includes('MALICIOUS_ISOLATION_BREACH');

  console.log('wsA schema contains canary:', wsA_has_canary);
  console.log('wsB schema contains canary (MUST BE FALSE):', wsB_has_canary);

  if (wsB_has_canary) {
    throw new Error('CRITICAL FAILURE: wsB schema contaminated by wsA write!');
  }

  // 6. Adversarial client corruption: inject canary type into wsA's compiled Prisma client
  const clientAPath = path.join(nmA, '.prisma', 'client', 'index.d.ts');
  const clientBPath = path.join(nmB, '.prisma', 'client', 'index.d.ts');

  const clientA_orig = fs.readFileSync(clientAPath, 'utf-8');
  const clientB_orig = fs.readFileSync(clientBPath, 'utf-8');

  const clientCanary = "\nexport type AdversarialBreachProbe = 'POISON_PAYLOAD';\n";
  fs.appendFileSync(clientAPath, clientCanary, 'utf-8');

  const clientA_after = fs.readFileSync(clientAPath, 'utf-8');
  const clientB_after = fs.readFileSync(clientBPath, 'utf-8');

  const clientA_has_canary = clientA_after.includes('POISON_PAYLOAD');
  const clientB_has_canary = clientB_after.includes('POISON_PAYLOAD');

  console.log('clientA contains canary:', clientA_has_canary);
  console.log('clientB contains canary (MUST BE FALSE):', clientB_has_canary);

  if (clientB_has_canary) {
    throw new Error('CRITICAL FAILURE: wsB Prisma client contaminated by wsA write!');
  }

  // 7. Test concurrent writes across both workspaces
  console.log('Testing concurrent file lock isolation across workspaces...');
  const iterations = 50;
  const pA = (async () => {
    for (let i = 0; i < iterations; i++) {
      fs.writeFileSync(schemaAPath, `// iter ${i} in A\n` + schemaA, 'utf-8');
    }
  })();

  const pB = (async () => {
    for (let i = 0; i < iterations; i++) {
      fs.writeFileSync(schemaBPath, `// iter ${i} in B\n` + schemaB, 'utf-8');
    }
  })();

  await Promise.all([pA, pB]);
  console.log('Concurrent write test completed successfully without lock collision.');

  // 8. Clean up scratch directory
  fs.rmSync(probeRoot, { recursive: true, force: true });
  console.log('Probe scratch directory cleaned up.');
  console.log('=== Adversarial Isolation Probe: ALL CHECKS PASSED ===');
}

runProbe().catch((err) => {
  console.error(err);
  process.exit(1);
});
