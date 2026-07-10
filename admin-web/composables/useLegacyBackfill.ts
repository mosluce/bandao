import type {
  LegacyBackfillJobDto,
  LegacyBackfillInput,
  LegacyBackfillPreviewRequest,
  LegacyBackfillPreviewResponse,
  LegacyBackfillSummaryDto,
} from '~/types/api'

/**
 * Wraps the legacy check-in backfill admin endpoints:
 * `POST /orgs/me/legacy-backfill` (save config),
 * `POST /orgs/me/legacy-backfill/preview` (dry-run, no writes), and
 * `GET /orgs/me/legacy-backfill/jobs` (read-only job status list).
 */
export function useLegacyBackfill() {
  const api = useApi()

  async function configure(req: LegacyBackfillInput): Promise<LegacyBackfillSummaryDto> {
    return api<LegacyBackfillSummaryDto>('/orgs/me/legacy-backfill', {
      method: 'POST',
      body: req,
    })
  }

  async function preview(req: LegacyBackfillPreviewRequest): Promise<LegacyBackfillPreviewResponse> {
    return api<LegacyBackfillPreviewResponse>('/orgs/me/legacy-backfill/preview', {
      method: 'POST',
      body: req,
    })
  }

  async function listJobs(): Promise<LegacyBackfillJobDto[]> {
    return api<LegacyBackfillJobDto[]>('/orgs/me/legacy-backfill/jobs')
  }

  return { configure, preview, listJobs }
}
