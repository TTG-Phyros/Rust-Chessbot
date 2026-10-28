# SelfplayJob

A self-play job.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**dataset** | **UUID** | Dataset filled by the games. | 
**games** | **int** | Games to play. | 
**network** | **UUID** | Network playing the games; &#x60;None&#x60;: the classical evaluation. | [optional] 
**nodes** | **int** | Nodes searched per move. | 
**seed** | **int** | Seed of the random openings. | 
**threads** | **int** | Worker threads (0: all cores). | 

## Example

```python
from chess_nnue_lab.models.selfplay_job import SelfplayJob

# TODO update the JSON string below
json = "{}"
# create an instance of SelfplayJob from a JSON string
selfplay_job_instance = SelfplayJob.from_json(json)
# print the JSON string representation of the object
print(SelfplayJob.to_json())

# convert the object into a dict
selfplay_job_dict = selfplay_job_instance.to_dict()
# create an instance of SelfplayJob from a dict
selfplay_job_from_dict = SelfplayJob.from_dict(selfplay_job_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


