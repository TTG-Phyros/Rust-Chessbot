
# Model

A trained network.

## Properties

Name | Type
------------ | -------------
`architecture` | [Architecture](Architecture.md)
`architectureHash` | string
`bytes` | number
`createdAt` | number
`fileSha256` | string
`id` | string
`job` | string
`name` | string
`provenance` | [Provenance](Provenance.md)

## Example

```typescript
import type { Model } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "architecture": null,
  "architectureHash": null,
  "bytes": null,
  "createdAt": null,
  "fileSha256": null,
  "id": null,
  "job": null,
  "name": null,
  "provenance": null,
} satisfies Model

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as Model
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


