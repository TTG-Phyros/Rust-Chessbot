
# Dataset

A dataset.

## Properties

Name | Type
------------ | -------------
`bytes` | number
`createdAt` | number
`error` | string
`id` | string
`job` | string
`name` | string
`records` | number
`sha256` | string
`source` | [DatasetSource](DatasetSource.md)
`stats` | [DatasetStats](DatasetStats.md)
`status` | [DatasetStatus](DatasetStatus.md)
`updatedAt` | number

## Example

```typescript
import type { Dataset } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "bytes": null,
  "createdAt": null,
  "error": null,
  "id": null,
  "job": null,
  "name": null,
  "records": null,
  "sha256": null,
  "source": null,
  "stats": null,
  "status": null,
  "updatedAt": null,
} satisfies Dataset

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as Dataset
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


