
# TrainingRequest

A training job, as a client asks for it.

## Properties

Name | Type
------------ | -------------
`architecture` | [Architecture](Architecture.md)
`architectureId` | string
`datasets` | Array&lt;string&gt;
`device` | [Device](Device.md)
`hyperparameters` | [Hyperparameters](Hyperparameters.md)
`name` | string
`seed` | number
`trainer` | string

## Example

```typescript
import type { TrainingRequest } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "architecture": null,
  "architectureId": null,
  "datasets": null,
  "device": null,
  "hyperparameters": null,
  "name": null,
  "seed": null,
  "trainer": null,
} satisfies TrainingRequest

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as TrainingRequest
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


