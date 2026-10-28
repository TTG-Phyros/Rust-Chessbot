
# Hyperparameters

How a network is trained. Every field has a default.

## Properties

Name | Type
------------ | -------------
`batchSize` | number
`batchesPerSuperbatch` | number
`learningRate` | number
`lrDropEvery` | number
`lrDropFactor` | number
`scoreScale` | number
`shuffleRecords` | number
`superbatches` | number
`wdl` | number

## Example

```typescript
import type { Hyperparameters } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "batchSize": null,
  "batchesPerSuperbatch": null,
  "learningRate": null,
  "lrDropEvery": null,
  "lrDropFactor": null,
  "scoreScale": null,
  "shuffleRecords": null,
  "superbatches": null,
  "wdl": null,
} satisfies Hyperparameters

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as Hyperparameters
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


