
# Layout

Resolved shapes of a (normalized) architecture.

## Properties

Name | Type
------------ | -------------
`accumulator` | number
`accumulatorActivation` | [Activation](Activation.md)
`dense` | [Array&lt;DenseShape&gt;](DenseShape.md)
`featureSet` | [FeatureSet](FeatureSet.md)
`ftOutputs` | number
`ftWeightMax` | number
`inputs` | number
`kingBuckets` | number
`kingRefreshes` | boolean
`maxActive` | number
`outputBuckets` | number
`perspectives` | number
`planes` | number
`weightMax` | number

## Example

```typescript
import type { Layout } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "accumulator": null,
  "accumulatorActivation": null,
  "dense": null,
  "featureSet": null,
  "ftOutputs": null,
  "ftWeightMax": null,
  "inputs": null,
  "kingBuckets": null,
  "kingRefreshes": null,
  "maxActive": null,
  "outputBuckets": null,
  "perspectives": null,
  "planes": null,
  "weightMax": null,
} satisfies Layout

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as Layout
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


