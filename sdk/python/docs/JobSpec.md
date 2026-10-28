# JobSpec

What a job does, with all its parameters.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**architecture** | [**Architecture**](Architecture.md) |  | 
**architecture_hash** | **str** | Its hash. | 
**datasets** | [**List[DatasetRef]**](DatasetRef.md) | Training data. | 
**device** | [**Device**](Device.md) | Where it may run. | 
**hyperparameters** | [**Hyperparameters**](Hyperparameters.md) | Hyperparameters. | 
**seed** | **int** | Seed of the random openings. | 
**trainer** | **str** | Trainer backend (&#x60;burn&#x60;). | 
**kind** | **str** |  | 
**dataset** | **UUID** | Dataset filled by the games. | 
**games** | **int** | Games to play at most (an even number). | 
**network** | **UUID** | Network playing the games; &#x60;None&#x60;: the classical evaluation. | [optional] 
**nodes** | **int** | Nodes searched per move (reproducible), unless &#x60;moveTimeMs&#x60;. | 
**threads** | **int** | Worker threads (0: all cores). | 
**baseline** | [**Contestant**](Contestant.md) | The engine it is measured against. | 
**candidate** | [**Contestant**](Contestant.md) | The engine evaluated. | 
**move_time_ms** | **int** | Milliseconds per move, instead of nodes. | [optional] 
**sprt** | [**Sprt**](Sprt.md) | Stops as soon as this test concludes; without it, every game is played. | [optional] 

## Example

```python
from chess_nnue_lab.models.job_spec import JobSpec

# TODO update the JSON string below
json = "{}"
# create an instance of JobSpec from a JSON string
job_spec_instance = JobSpec.from_json(json)
# print the JSON string representation of the object
print(JobSpec.to_json())

# convert the object into a dict
job_spec_dict = job_spec_instance.to_dict()
# create an instance of JobSpec from a dict
job_spec_from_dict = JobSpec.from_dict(job_spec_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


