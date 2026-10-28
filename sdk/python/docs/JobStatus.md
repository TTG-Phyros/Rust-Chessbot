# JobStatus

Where a job is in its life.  ```text queued ──claim──▶ running ──▶ succeeded | failed   │                 │ stop          │ worker lost (no heartbeat)   ▼                 ▼               ▼ canceled         stopping ──▶ stopped     interrupted                                  └──── resume ───┴──▶ queued ```  `failed` training jobs can be resumed too (from their last checkpoint).

## Enum

* `QUEUED` (value: `'queued'`)

* `RUNNING` (value: `'running'`)

* `STOPPING` (value: `'stopping'`)

* `STOPPED` (value: `'stopped'`)

* `INTERRUPTED` (value: `'interrupted'`)

* `SUCCEEDED` (value: `'succeeded'`)

* `FAILED` (value: `'failed'`)

* `CANCELED` (value: `'canceled'`)

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


