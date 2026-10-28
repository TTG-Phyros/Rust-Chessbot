
# ValidationReport

Result of a validation: the errors, or what the architecture resolves to.

## Properties

Name | Type
------------ | -------------
`errors` | [Array&lt;FieldError&gt;](FieldError.md)
`estimate` | [Estimate](Estimate.md)
`hash` | string
`layout` | [Layout](Layout.md)
`normalized` | [Architecture](Architecture.md)
`valid` | boolean

## Example

```typescript
import type { ValidationReport } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "errors": null,
  "estimate": null,
  "hash": null,
  "layout": null,
  "normalized": null,
  "valid": null,
} satisfies ValidationReport

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as ValidationReport
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


