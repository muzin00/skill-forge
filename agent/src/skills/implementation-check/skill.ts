import { defineTask } from '../../lib/defineTask.js';

interface ImplementationCheckInput {
  issueNumber: string;
}

interface ImplementationCheckResult {
  evaluation: string;
  implementable: boolean;
}

defineTask<ImplementationCheckInput, ImplementationCheckResult>({
  allowSkills: ['view-issue', 'read-file', 'grep-file'],
});
