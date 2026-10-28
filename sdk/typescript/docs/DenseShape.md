
# DenseShape

A dense layer as stored: `buckets` sets of `outputs × inputs` weights.

## Properties

Name | Type
------------ | -------------
`activation` | [Activation](Activation.md)
`buckets` | number
`inputBound` | number
`inputs` | number
`outputs` | number
`sum` | [SumWidth](SumWidth.md)

## Example

```typescript
import type { DenseShape } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "activation": null,
  "buckets": null,
  "inputBound": null,
  "inputs": null,
  "outputs": null,
  "sum": null,
} satisfies DenseShape

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as DenseShape
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


