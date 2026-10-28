# JobEvent

Something that happened to a job, in order (`seq` grows across jobs).

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**data** | **object** | Details, depending on &#x60;kind&#x60;. | 
**job** | **UUID** | The job. | 
**kind** | [**EventKind**](EventKind.md) | What. | 
**seq** | **int** | Position in the log of all events. | 
**time** | **int** | When (Unix milliseconds). | 

## Example

```python
from chess_nnue_lab.models.job_event import JobEvent

# TODO update the JSON string below
json = "{}"
# create an instance of JobEvent from a JSON string
job_event_instance = JobEvent.from_json(json)
# print the JSON string representation of the object
print(JobEvent.to_json())

# convert the object into a dict
job_event_dict = job_event_instance.to_dict()
# create an instance of JobEvent from a dict
job_event_from_dict = JobEvent.from_dict(job_event_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


