# ProgressOneOf2

See [`EvaluationProgress`].

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**baseline_nps** | **int** | Nodes per second of the baseline&#39;s search, on the same worker. | [optional] 
**candidate_nps** | **int** | Nodes per second of the candidate&#39;s search (one thread, measured by the worker before the match). | [optional] 
**draws** | **int** | Games drawn. | 
**elo** | **float** | Elo difference of the candidate over the baseline. | 
**elo_error** | **float** | Half-width of the 95 % confidence interval of &#x60;elo&#x60;. | 
**games** | **int** | Games played. | 
**games_max** | **int** | Games to play at most. | 
**los** | **float** | Likelihood of superiority: probability that the candidate is stronger. | 
**losses** | **int** | Games the candidate lost. | 
**score** | **float** | Candidate&#39;s mean score, in [0, 1]. | 
**sprt** | [**SprtStatus**](SprtStatus.md) | The SPRT, when the match runs one. | [optional] 
**wins** | **int** | Games the candidate won. | 
**kind** | **str** |  | 

## Example

```python
from chess_nnue_lab.models.progress_one_of2 import ProgressOneOf2

# TODO update the JSON string below
json = "{}"
# create an instance of ProgressOneOf2 from a JSON string
progress_one_of2_instance = ProgressOneOf2.from_json(json)
# print the JSON string representation of the object
print(ProgressOneOf2.to_json())

# convert the object into a dict
progress_one_of2_dict = progress_one_of2_instance.to_dict()
# create an instance of ProgressOneOf2 from a dict
progress_one_of2_from_dict = ProgressOneOf2.from_dict(progress_one_of2_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


