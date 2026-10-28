# EvaluationProgress

Latest result of an evaluation match, from the candidate's side.

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

## Example

```python
from chess_nnue_lab.models.evaluation_progress import EvaluationProgress

# TODO update the JSON string below
json = "{}"
# create an instance of EvaluationProgress from a JSON string
evaluation_progress_instance = EvaluationProgress.from_json(json)
# print the JSON string representation of the object
print(EvaluationProgress.to_json())

# convert the object into a dict
evaluation_progress_dict = evaluation_progress_instance.to_dict()
# create an instance of EvaluationProgress from a dict
evaluation_progress_from_dict = EvaluationProgress.from_dict(evaluation_progress_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


