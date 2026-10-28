
# PairResult

The games between two players of a comparison.

## Properties

Name | Type
------------ | -------------
`a` | number
`b` | number
`draws` | number
`elo` | number
`eloError` | number
`evaluations` | Array&lt;string&gt;
`los` | number
`losses` | number
`wins` | number

## Example

```typescript
import type { PairResult } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "a": null,
  "b": null,
  "draws": null,
  "elo": null,
  "eloError": null,
  "evaluations": null,
  "los": null,
  "losses": null,
  "wins": null,
} satisfies PairResult

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as PairResult
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


