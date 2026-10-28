
# Job

A job.

## Properties

Name | Type
------------ | -------------
`attempt` | number
`createdAt` | number
`error` | string
`finishedAt` | number
`id` | string
`model` | string
`name` | string
`progress` | [Progress](Progress.md)
`spec` | [JobSpec](JobSpec.md)
`startedAt` | number
`status` | [JobStatus](JobStatus.md)
`updatedAt` | number
`worker` | string

## Example

```typescript
import type { Job } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "attempt": null,
  "createdAt": null,
  "error": null,
  "finishedAt": null,
  "id": null,
  "model": null,
  "name": null,
  "progress": null,
  "spec": null,
  "startedAt": null,
  "status": null,
  "updatedAt": null,
  "worker": null,
} satisfies Job

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as Job
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


