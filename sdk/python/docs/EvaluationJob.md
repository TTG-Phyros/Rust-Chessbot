# EvaluationJob

An evaluation match, fully resolved: the candidate plays pairs of games against the baseline from the same random opening with colors swapped.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**baseline** | [**Contestant**](Contestant.md) | The engine it is measured against. | 
**candidate** | [**Contestant**](Contestant.md) | The engine evaluated. | 
**games** | **int** | Games to play at most (an even number). | 
**move_time_ms** | **int** | Milliseconds per move, instead of nodes. | [optional] 
**nodes** | **int** | Nodes searched per move (reproducible), unless &#x60;moveTimeMs&#x60;. | [optional] 
**seed** | **int** | Seed of the random openings. | 
**sprt** | [**Sprt**](Sprt.md) | Stops as soon as this test concludes; without it, every game is played. | [optional] 
**threads** | **int** | Worker threads (0: all cores). | 

## Example

```python
from chess_nnue_lab.models.evaluation_job import EvaluationJob

# TODO update the JSON string below
json = "{}"
# create an instance of EvaluationJob from a JSON string
evaluation_job_instance = EvaluationJob.from_json(json)
# print the JSON string representation of the object
print(EvaluationJob.to_json())

# convert the object into a dict
evaluation_job_dict = evaluation_job_instance.to_dict()
# create an instance of EvaluationJob from a dict
evaluation_job_from_dict = EvaluationJob.from_dict(evaluation_job_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


