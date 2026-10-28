
# DatasetStats

What a dataset contains.

## Properties

Name | Type
------------ | -------------
`meanAbsScore` | number
`outcomes` | [Outcomes](Outcomes.md)
`pieces` | Array&lt;number&gt;
`plies` | [Histogram](Histogram.md)
`records` | number
`scores` | [Histogram](Histogram.md)
`whiteToMove` | number

## Example

```typescript
import type { DatasetStats } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "meanAbsScore": null,
  "outcomes": null,
  "pieces": null,
  "plies": null,
  "records": null,
  "scores": null,
  "whiteToMove": null,
} satisfies DatasetStats

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as DatasetStats
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


