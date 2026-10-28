
# DatasetRef

A dataset a training job reads, as it was when the job was created.

## Properties

Name | Type
------------ | -------------
`id` | string
`records` | number
`sha256` | string

## Example

```typescript
import type { DatasetRef } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "id": null,
  "records": null,
  "sha256": null,
} satisfies DatasetRef

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as DatasetRef
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


