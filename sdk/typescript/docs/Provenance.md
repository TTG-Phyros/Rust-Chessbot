
# Provenance

How a model was made: enough to train it again.

## Properties

Name | Type
------------ | -------------
`datasets` | [Array&lt;DatasetRef&gt;](DatasetRef.md)
`device` | string
`finalLoss` | number
`hyperparameters` | [Hyperparameters](Hyperparameters.md)
`positions` | number
`seed` | number
`superbatches` | number
`trainer` | string

## Example

```typescript
import type { Provenance } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "datasets": null,
  "device": null,
  "finalLoss": null,
  "hyperparameters": null,
  "positions": null,
  "seed": null,
  "superbatches": null,
  "trainer": null,
} satisfies Provenance

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as Provenance
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


