
# OutputBuckets

Output buckets.

## Properties

Name | Type
------------ | -------------
`count` | number
`scope` | [BucketScope](BucketScope.md)
`strategy` | [BucketStrategy](BucketStrategy.md)

## Example

```typescript
import type { OutputBuckets } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "count": null,
  "scope": null,
  "strategy": null,
} satisfies OutputBuckets

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as OutputBuckets
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


