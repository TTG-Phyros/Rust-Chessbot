
# Problem

An error (RFC 9457). `type` identifies the kind of problem (`urn:chess-bot:problem:<name>`); validation problems list every error found in `errors`.

## Properties

Name | Type
------------ | -------------
`detail` | string
`errors` | [Array&lt;FieldError&gt;](FieldError.md)
`status` | number
`title` | string
`type` | string

## Example

```typescript
import type { Problem } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "detail": null,
  "errors": null,
  "status": null,
  "title": null,
  "type": null,
} satisfies Problem

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as Problem
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


