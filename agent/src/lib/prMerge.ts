declare const execCmd: (cmd: string, args: string[]) => Promise<string>;
declare function log(message: string): void;

export interface PrMergeInput {}

export interface PrMergeOutput {
  merged: boolean;
  prUrl?: string;
}

interface CheckItem {
  __typename?: string;
  name?: string;
  context?: string;
  status?: string;
  conclusion?: string;
  state?: string;
}

interface PrInfo {
  state: 'OPEN' | 'CLOSED' | 'MERGED';
  url: string;
  statusCheckRollup?: CheckItem[];
}

const FAILURE_CONCLUSIONS = new Set([
  'FAILURE',
  'CANCELLED',
  'TIMED_OUT',
  'ACTION_REQUIRED',
  'STARTUP_FAILURE',
]);
const FAILURE_STATES = new Set(['FAILURE', 'ERROR']);
const PENDING_STATES = new Set(['PENDING', 'EXPECTED']);

type CiStatus = 'pass' | 'fail' | 'pending';

function classifyChecks(items: CheckItem[]): CiStatus {
  let pending = false;
  for (const item of items) {
    if (item.status !== undefined) {
      if (item.status !== 'COMPLETED') {
        pending = true;
        continue;
      }
      if (item.conclusion && FAILURE_CONCLUSIONS.has(item.conclusion)) {
        return 'fail';
      }
    } else if (item.state !== undefined) {
      if (FAILURE_STATES.has(item.state)) return 'fail';
      if (PENDING_STATES.has(item.state)) pending = true;
    }
  }
  return pending ? 'pending' : 'pass';
}

async function fetchPr(): Promise<PrInfo> {
  const json = await execCmd('gh', [
    'pr',
    'view',
    '--json',
    'state,url,statusCheckRollup',
  ]);
  return JSON.parse(json) as PrInfo;
}

export async function prMerge(_input: PrMergeInput): Promise<PrMergeOutput> {
  const pr = await fetchPr();

  if (pr.state !== 'OPEN') {
    log(`PR is not open (state=${pr.state}); skipping merge.`);
    return { merged: false, prUrl: pr.url };
  }

  let status = classifyChecks(pr.statusCheckRollup ?? []);
  if (status === 'pending') {
    log('CI checks in progress; waiting via `gh pr checks --watch`...');
    await execCmd('gh', ['pr', 'checks', '--watch']);
    const refreshed = await fetchPr();
    status = classifyChecks(refreshed.statusCheckRollup ?? []);
  }

  if (status === 'fail') {
    log('CI checks failed; skipping merge.');
    return { merged: false, prUrl: pr.url };
  }

  log('CI checks passed; merging...');
  await execCmd('gh', ['pr', 'merge', '--squash', '--delete-branch']);

  log('Switching to main and pulling latest...');
  await execCmd('git', ['checkout', 'main']);
  await execCmd('git', ['pull']);

  return { merged: true, prUrl: pr.url };
}
