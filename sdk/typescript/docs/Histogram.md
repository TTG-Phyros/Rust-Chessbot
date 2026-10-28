
# Histogram

Counts of values in bins of equal width, with the values below the first bin in `below` and those past the last in `above`.

## Properties

Name | Type
------------ | -------------
`above` | number
`below` | number
`counts` | Array&lt;number&gt;
`min` | number
`width` | number

## Example

```typescript
import type { Histogram } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "above": null,
  "below": null,
  "counts": null,
  "min": null,
  "width": null,
} satisfies Histogram

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as Histogram
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


