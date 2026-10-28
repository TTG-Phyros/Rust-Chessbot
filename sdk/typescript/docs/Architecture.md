
# Architecture

A complete NNUE architecture: input features, feature transformer (accumulator), dense layers, output buckets and quantization.

## Properties

Name | Type
------------ | -------------
`$schema` | string
`accumulator` | [Accumulator](Accumulator.md)
`features` | [Features](Features.md)
`hiddenLayers` | [Array&lt;DenseLayer&gt;](DenseLayer.md)
`metadata` | [Metadata](Metadata.md)
`outputBuckets` | [OutputBuckets](OutputBuckets.md)
`quantization` | [Quantization](Quantization.md)
`schemaVersion` | number

## Example

```typescript
import type { Architecture } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "$schema": null,
  "accumulator": null,
  "features": null,
  "hiddenLayers": null,
  "metadata": null,
  "outputBuckets": null,
  "quantization": null,
  "schemaVersion": null,
} satisfies Architecture

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as Architecture
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


