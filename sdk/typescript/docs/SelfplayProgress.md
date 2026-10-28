
# SelfplayProgress

Latest progress of a self-play job.

## Properties

Name | Type
------------ | -------------
`games` | number
`gamesTotal` | number
`positions` | number

## Example

```typescript
import type { SelfplayProgress } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "games": null,
  "gamesTotal": null,
  "positions": null,
} satisfies SelfplayProgress

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as SelfplayProgress
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


