# PairResult

The games between two players of a comparison.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**a** | **int** | Index of the first player in &#x60;players&#x60;. | 
**b** | **int** | Index of the second player (always greater than &#x60;a&#x60;). | 
**draws** | **int** | Games drawn. | 
**elo** | **float** | Elo difference of &#x60;a&#x60; over &#x60;b&#x60;. | 
**elo_error** | **float** | Half-width of the 95 % confidence interval of &#x60;elo&#x60;. | 
**evaluations** | **List[UUID]** | Evaluations whose games are counted. | 
**los** | **float** | Likelihood that &#x60;a&#x60; is the stronger. | 
**losses** | **int** | Games &#x60;a&#x60; lost. | 
**wins** | **int** | Games &#x60;a&#x60; won. | 

## Example

```python
from chess_nnue_lab.models.pair_result import PairResult

# TODO update the JSON string below
json = "{}"
# create an instance of PairResult from a JSON string
pair_result_instance = PairResult.from_json(json)
# print the JSON string representation of the object
print(PairResult.to_json())

# convert the object into a dict
pair_result_dict = pair_result_instance.to_dict()
# create an instance of PairResult from a dict
pair_result_from_dict = PairResult.from_dict(pair_result_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


