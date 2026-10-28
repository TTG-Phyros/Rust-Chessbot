
# JobEvent

Something that happened to a job, in order (`seq` grows across jobs).

## Properties

Name | Type
------------ | -------------
`data` | any
`job` | string
`kind` | [EventKind](EventKind.md)
`seq` | number
`time` | number

## Example

```typescript
import type { JobEvent } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "data": null,
  "job": null,
  "kind": null,
  "seq": null,
  "time": null,
} satisfies JobEvent

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as JobEvent
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


