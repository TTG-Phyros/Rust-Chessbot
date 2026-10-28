
# BucketStrategy

Bucket selection. - `none`: A single set of weights. - `pieceCount`: `(pieces − 2) / (32 / count)`, pieces counted with the kings. - `material`: `min(count − 1, material · count / 79)`, material counted P=1, N=B=3, R=5, Q=9 for both sides (78 at the start).

## Properties

Name | Type
------------ | -------------

## Example

```typescript
import type { BucketStrategy } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
} satisfies BucketStrategy

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as BucketStrategy
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


