
# Quantization

Integer storage of the network.  Accumulator values are stored ×`qa` (1.0 ↔ `qa`), dense weights ×`qb`, dense biases ×`qa·qb` in i32. Every activation brings values back to the `qa` scale, and the network output is `sum · evalScale / (qa · qb)` centipawns.

## Properties

Name | Type
------------ | -------------
`accumulator` | [AccumulatorType](AccumulatorType.md)
`evalScale` | number
`ftWeightClip` | number
`qa` | number
`qb` | number
`weightClip` | number
`weights` | [WeightType](WeightType.md)

## Example

```typescript
import type { Quantization } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "accumulator": null,
  "evalScale": null,
  "ftWeightClip": null,
  "qa": null,
  "qb": null,
  "weightClip": null,
  "weights": null,
} satisfies Quantization

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as Quantization
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


