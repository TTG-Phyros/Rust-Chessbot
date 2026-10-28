
# Metrics

The training curve of a run: one point per superbatch (resumed runs may repeat superbatches trained again after a checkpoint).

## Properties

Name | Type
------------ | -------------
`points` | [Array&lt;MetricPoint&gt;](MetricPoint.md)

## Example

```typescript
import type { Metrics } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "points": null,
} satisfies Metrics

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as Metrics
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


