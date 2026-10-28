
# MetricPoint

One point of a training curve.

## Properties

Name | Type
------------ | -------------
`learningRate` | number
`loss` | number
`positions` | number
`positionsPerSecond` | number
`step` | number
`superbatch` | number
`superbatches` | number
`vramUsedMb` | number
`time` | number

## Example

```typescript
import type { MetricPoint } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "learningRate": null,
  "loss": null,
  "positions": null,
  "positionsPerSecond": null,
  "step": null,
  "superbatch": null,
  "superbatches": null,
  "vramUsedMb": null,
  "time": null,
} satisfies MetricPoint

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as MetricPoint
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


