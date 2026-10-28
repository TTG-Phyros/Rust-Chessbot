
# Metadata

Descriptive information about a network.

## Properties

Name | Type
------------ | -------------
`author` | string
`description` | string
`name` | string
`tags` | Array&lt;string&gt;
`version` | string

## Example

```typescript
import type { Metadata } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "author": null,
  "description": null,
  "name": null,
  "tags": null,
  "version": null,
} satisfies Metadata

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as Metadata
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


