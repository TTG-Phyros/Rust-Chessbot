# Comparison

Several players compared over the evaluations between them.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**pairs** | [**List[PairResult]**](PairResult.md) | Every pair that played, &#x60;a&#x60; before &#x60;b&#x60;. | 
**players** | [**List[ComparedPlayer]**](ComparedPlayer.md) | The players, in the order asked. | 

## Example

```python
from chess_nnue_lab.models.comparison import Comparison

# TODO update the JSON string below
json = "{}"
# create an instance of Comparison from a JSON string
comparison_instance = Comparison.from_json(json)
# print the JSON string representation of the object
print(Comparison.to_json())

# convert the object into a dict
comparison_dict = comparison_instance.to_dict()
# create an instance of Comparison from a dict
comparison_from_dict = Comparison.from_dict(comparison_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


