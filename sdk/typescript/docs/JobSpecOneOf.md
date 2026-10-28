
# JobSpecOneOf

See [`TrainingJob`].

## Properties

Name | Type
------------ | -------------
`architecture` | [Architecture](Architecture.md)
`architectureHash` | string
`datasets` | [Array&lt;DatasetRef&gt;](DatasetRef.md)
`device` | [Device](Device.md)
`hyperparameters` | [Hyperparameters](Hyperparameters.md)
`seed` | number
`trainer` | string
`kind` | string

## Example

```typescript
import type { JobSpecOneOf } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "architecture": null,
  "architectureHash": null,
  "datasets": null,
  "device": null,
  "hyperparameters": null,
  "seed": null,
  "trainer": null,
  "kind": null,
} satisfies JobSpecOneOf

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as JobSpecOneOf
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


