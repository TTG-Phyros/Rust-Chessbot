# JobSpecOneOf1

See [`SelfplayJob`].

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**dataset** | **UUID** | Dataset filled by the games. | 
**games** | **int** | Games to play. | 
**network** | **UUID** | Network playing the games; &#x60;None&#x60;: the classical evaluation. | [optional] 
**nodes** | **int** | Nodes searched per move. | 
**seed** | **int** | Seed of the random openings. | 
**threads** | **int** | Worker threads (0: all cores). | 
**kind** | **str** |  | 

## Example

```python
from chess_nnue_lab.models.job_spec_one_of1 import JobSpecOneOf1

# TODO update the JSON string below
json = "{}"
# create an instance of JobSpecOneOf1 from a JSON string
job_spec_one_of1_instance = JobSpecOneOf1.from_json(json)
# print the JSON string representation of the object
print(JobSpecOneOf1.to_json())

# convert the object into a dict
job_spec_one_of1_dict = job_spec_one_of1_instance.to_dict()
# create an instance of JobSpecOneOf1 from a dict
job_spec_one_of1_from_dict = JobSpecOneOf1.from_dict(job_spec_one_of1_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


