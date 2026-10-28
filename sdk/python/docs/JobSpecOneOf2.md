# JobSpecOneOf2

See [`EvaluationJob`].

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
**kind** | **str** |  | 

## Example

```python
from chess_nnue_lab.models.job_spec_one_of2 import JobSpecOneOf2

# TODO update the JSON string below
json = "{}"
# create an instance of JobSpecOneOf2 from a JSON string
job_spec_one_of2_instance = JobSpecOneOf2.from_json(json)
# print the JSON string representation of the object
print(JobSpecOneOf2.to_json())

# convert the object into a dict
job_spec_one_of2_dict = job_spec_one_of2_instance.to_dict()
# create an instance of JobSpecOneOf2 from a dict
job_spec_one_of2_from_dict = JobSpecOneOf2.from_dict(job_spec_one_of2_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


