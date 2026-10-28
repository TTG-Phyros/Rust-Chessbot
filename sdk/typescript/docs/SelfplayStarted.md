
# SelfplayStarted

A self-play run and the dataset it fills.

## Properties

Name | Type
------------ | -------------
`dataset` | [Dataset](Dataset.md)
`run` | [Job](Job.md)

## Example

```typescript
import type { SelfplayStarted } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "dataset": null,
  "run": null,
} satisfies SelfplayStarted

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as SelfplayStarted
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


