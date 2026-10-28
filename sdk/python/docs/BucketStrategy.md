# BucketStrategy

Bucket selection. - `none`: A single set of weights. - `pieceCount`: `(pieces − 2) / (32 / count)`, pieces counted with the kings. - `material`: `min(count − 1, material · count / 79)`, material counted P=1, N=B=3, R=5, Q=9 for both sides (78 at the start).

## Enum

* `NONE` (value: `'none'`)

* `PIECECOUNT` (value: `'pieceCount'`)

* `MATERIAL` (value: `'material'`)

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


