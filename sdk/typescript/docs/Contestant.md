
# Contestant

A player of an evaluation match: a model, or the classical evaluation.

## Properties

Name | Type
------------ | -------------
`architectureHash` | string
`bytes` | number
`model` | string
`name` | string
`parameters` | number

## Example

```typescript
import type { Contestant } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "architectureHash": null,
  "bytes": null,
  "model": null,
  "name": null,
  "parameters": null,
} satisfies Contestant

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as Contestant
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


