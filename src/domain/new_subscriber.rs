use crate::domain::SubscriberName;
use crate::domain::SubscriberEmail;

pub struct NewSubscriber {
    //String은 더 이상 사용하지 않는다.
    pub email: SubscriberEmail,
    pub name: SubscriberName
}